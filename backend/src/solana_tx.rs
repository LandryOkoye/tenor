//! Solana transaction layer: assembly, safety validation, simulation,
//! broadcast and confirmation (brief §7.3, §13).
//!
//! Responsibilities:
//! 1. Convert Panta's wire instructions (`{programId, data, accounts}`) into
//!    real `solana_sdk::Instruction`s.
//! 2. Add our own instructions to a purchase transaction:
//!    - ComputeBudget: set_compute_unit_limit + set_compute_unit_price
//!    - our fee: an SPL `transfer_checked` of USDC from the buyer's associated
//!      token account (ATA) to the fee wallet's ATA
//! 3. Assemble ONE versioned (v0) transaction with a fresh blockhash and hand
//!    it back **unsigned** (base64) for the user's wallet to sign.
//! 4. `validate()` — the funds-protection check. Before any unsigned tx is
//!    returned to a user, verify:
//!    - every instruction's program is on a strict allowlist
//!      (System / Token / ATA / ComputeBudget / configured Panta programs)
//!    - the first signer (fee payer) is the user's own wallet
//!    - when a fee is expected: exactly one USDC transfer to OUR fee ATA,
//!      for the exact quoted amount, funded from the user's own USDC ATA
//! 5. `simulate()` on our RPC (skipped in mock/dev mode) so users never see
//!    a tx that fails on-chain.
//! 6. `broadcast_and_confirm()` — send the signed tx and poll until confirmed.
//!
//! Money note: the fee amount is integer USDC base units (6 decimals),
//! converted to raw token units for the SPL instruction — never floats.

use std::str::FromStr;
use std::time::Duration;

use base64::Engine;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};

use crate::config::Config;
use crate::panta::WireInstruction;

// --- Well-known Solana program ids (fixed constants, no discovery) ----------

/// SPL Token program (classic Tokenkeg — USDC lives here).
/// Verified: decodes to a valid 32-byte pubkey (solana-program-library docs).
pub const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// Associated Token Account program (derives the user/fee-wallet USDC ATAs).
/// Verified: decodes to a valid 32-byte pubkey (spl.solana.com docs).
pub const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
/// ComputeBudget program (CU limit + priority fee instructions).
pub const COMPUTE_BUDGET_PROGRAM: &str = "ComputeBudget111111111111111111111111111111";
/// System program (mock instructions in dev mode; never expected in real buys).
pub const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

/// SPL Token `transfer_checked` instruction discriminant.
const SPL_TRANSFER_CHECKED: u8 = 12;
/// USDC (and all Panta settlement) uses 6 decimals.
const USDC_DECIMALS: u8 = 6;
/// Solana network packet size limit for a transaction, in bytes.
const MAX_TX_SIZE: usize = 1232;

// --- Errors -----------------------------------------------------------------

/// Everything that can go wrong in the transaction layer.
#[derive(Debug, thiserror::Error)]
pub enum TxError {
    #[error("invalid instruction: {0}")]
    Invalid(String),
    #[error("transaction too large: {size} bytes (limit {MAX_TX_SIZE})")]
    TooLarge { size: usize },
    #[error("rejected by safety validation: {0}")]
    Validation(String),
    #[error("simulation failed: {0}")]
    Simulation(String),
    #[error("rpc error: {0}")]
    Rpc(String),
    #[error("transaction expired before confirmation")]
    Expired,
    #[error("not confirmed within timeout")]
    NotConfirmed,
}

// --- Context ----------------------------------------------------------------

/// Everything the transaction layer needs, built once at startup.
pub struct SolanaCtx {
    rpc: RpcClient,
    /// USDC mint (mainnet USDC by default; overridable via USDC_MINT).
    usdc_mint: Pubkey,
    /// Fee wallet that receives our fee (its ATA is created ahead of launch).
    fee_wallet: Pubkey,
    /// Panta program ids allowed inside purchase transactions (real mode).
    panta_programs: Vec<Pubkey>,
    /// Priority fee (micro-lamports per compute unit) for ComputeBudget ix.
    priority_fee_microlamports: u64,
    /// Dev/mock mode: skip RPC simulation and fake signatures.
    mock: bool,
}

impl SolanaCtx {
    /// Build from parsed config values. Fails fast on invalid pubkeys.
    pub fn from_config(config: &Config) -> anyhow::Result<Self> {
        let usdc_mint = Pubkey::from_str(&config.usdc_mint)
            .map_err(|e| anyhow::anyhow!("USDC_MINT is not a valid pubkey: {e}"))?;

        // Dev mode may run without a fee wallet configured; production must have one.
        let fee_wallet = if config.fee_wallet_pubkey.is_empty() {
            anyhow::ensure!(
                !config.is_production(),
                "FEE_WALLET_PUBKEY must be set in production"
            );
            // Deterministic placeholder in dev (on-chain funds are never moved in mock mode).
            Pubkey::default()
        } else {
            Pubkey::from_str(&config.fee_wallet_pubkey)
                .map_err(|e| anyhow::anyhow!("FEE_WALLET_PUBKEY is not a valid pubkey: {e}"))?
        };

        let mut panta_programs = Vec::new();
        for id in &config.panta_program_ids {
            panta_programs.push(
                Pubkey::from_str(id)
                    .map_err(|e| anyhow::anyhow!("PANTA_PROGRAM_IDS entry {id:?} invalid: {e}"))?,
            );
        }

        Ok(Self {
            rpc: RpcClient::new(config.solana_rpc_url.clone()),
            usdc_mint,
            fee_wallet,
            panta_programs,
            priority_fee_microlamports: config.priority_fee_microlamports,
            mock: config.panta_mock,
        })
    }

    // -- address derivation ---------------------------------------------------

    /// The wallet's associated USDC token account (where its USDC sits).
    pub fn user_usdc_ata(&self, wallet: &Pubkey) -> Pubkey {
        Self::usdc_ata(wallet, &self.usdc_mint)
    }

    /// Our fee wallet's associated USDC token account (fee destination).
    pub fn fee_usdc_ata(&self) -> Pubkey {
        Self::usdc_ata(&self.fee_wallet, &self.usdc_mint)
    }

    /// Derive the canonical SPL associated token account for (owner, mint).
    /// PDA seeds per the ATA program: [owner, token_program, mint].
    fn usdc_ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
        let token_program = Pubkey::from_str(TOKEN_PROGRAM).expect("token program id");
        let ata_program = Pubkey::from_str(ATA_PROGRAM).expect("ata program id");
        Pubkey::find_program_address(
            &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
            &ata_program,
        )
        .0
    }

    // -- instruction building -------------------------------------------------

    /// Our fee instruction: SPL `transfer_checked` of `amount_base` USDC
    /// from the buyer's ATA to the fee wallet's ATA.
    ///
    /// Hand-rolled (instead of pulling in spl-token) so the instruction shape
    /// is explicit and reviewable: data = [12, amount u64-LE, decimals].
    pub fn fee_instruction(
        &self,
        payer: &Pubkey,
        amount_base: i64,
    ) -> Result<Instruction, TxError> {
        if amount_base < 0 {
            return Err(TxError::Invalid("fee amount must be non-negative".into()));
        }
        let token_program = Pubkey::from_str(TOKEN_PROGRAM).expect("token program id");
        Ok(Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(self.user_usdc_ata(payer), false), // source: buyer's USDC
                AccountMeta::new_readonly(self.usdc_mint, false),   // mint (decimals check)
                AccountMeta::new(self.fee_usdc_ata(), false),       // destination: fee wallet
                AccountMeta::new_readonly(*payer, true),            // authority: buyer signs
            ],
            data: [
                vec![SPL_TRANSFER_CHECKED],
                (amount_base as u64).to_le_bytes().to_vec(),
                vec![USDC_DECIMALS],
            ]
            .concat(),
        })
    }

    /// ComputeBudget instructions: a CU limit and a priority fee.
    /// Data layout: [tag, value-LE] — tag 1 = set_compute_unit_limit (u32),
    /// tag 2 = set_compute_unit_price (u64 micro-lamports/CU).
    fn compute_budget_instructions(&self, cu_limit: u32) -> Vec<Instruction> {
        let program = Pubkey::from_str(COMPUTE_BUDGET_PROGRAM).expect("compute budget id");
        vec![
            Instruction {
                program_id: program,
                accounts: vec![],
                data: [vec![1u8], cu_limit.to_le_bytes().to_vec()].concat(),
            },
            Instruction {
                program_id: program,
                accounts: vec![],
                data: [
                    vec![2u8],
                    self.priority_fee_microlamports.to_le_bytes().to_vec(),
                ]
                .concat(),
            },
        ]
    }

    /// Convert one Panta wire instruction into a real Solana instruction.
    /// Any malformed field (bad pubkey, bad base64) fails loudly.
    pub fn parse_wire_instruction(wire: &WireInstruction) -> Result<Instruction, TxError> {
        let program_id = Pubkey::from_str(&wire.program_id)
            .map_err(|e| TxError::Invalid(format!("programId {:?}: {e}", wire.program_id)))?;

        let data = base64::engine::general_purpose::STANDARD
            .decode(&wire.data)
            .map_err(|e| TxError::Invalid(format!("instruction data is not base64: {e}")))?;

        let mut accounts = Vec::with_capacity(wire.accounts.len());
        for acc in &wire.accounts {
            accounts.push(AccountMeta {
                pubkey: Pubkey::from_str(&acc.pubkey)
                    .map_err(|e| TxError::Invalid(format!("account pubkey: {e}")))?,
                is_signer: acc.is_signer,
                is_writable: acc.is_writable,
            });
        }

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }

    // -- assembly -------------------------------------------------------------

    /// Assemble one unsigned v0 transaction for `payer` containing:
    /// ComputeBudget instructions, all `leg_instructions` (in order), and —
    /// when `fee_base` is set — our fee transfer as the last instruction.
    ///
    /// Returns the transaction with **zeroed signatures**: the user's wallet
    /// fills them in when signing. Fetches a fresh blockhash from our RPC.
    pub async fn assemble(
        &self,
        payer: &Pubkey,
        leg_instructions: Vec<Vec<Instruction>>,
        fee_base: Option<i64>,
        cu_limit: u32,
    ) -> Result<VersionedTransaction, TxError> {
        // Fresh blockhash from OUR RPC (brief §7.3) — never Panta's hint.
        let blockhash = self
            .rpc
            .get_latest_blockhash()
            .await
            .map_err(|e| TxError::Rpc(format!("could not get blockhash: {e}")))?;
        self.assemble_with_blockhash(payer, leg_instructions, fee_base, cu_limit, blockhash)
    }

    /// Pure assembly (blockhash supplied) — separated so tests need no RPC.
    pub fn assemble_with_blockhash(
        &self,
        payer: &Pubkey,
        leg_instructions: Vec<Vec<Instruction>>,
        fee_base: Option<i64>,
        cu_limit: u32,
        blockhash: solana_sdk::hash::Hash,
    ) -> Result<VersionedTransaction, TxError> {
        // Instruction order matters on Solana: compute budget first, then the
        // legs, then our fee.
        let mut instructions = self.compute_budget_instructions(cu_limit);
        for leg in leg_instructions {
            instructions.extend(leg);
        }
        if let Some(fee) = fee_base {
            instructions.push(self.fee_instruction(payer, fee)?);
        }

        // Compile a v0 message with no address lookup tables (MVP baskets fit;
        // if a tx ever exceeds the packet size we return TX_TOO_LARGE).
        let message = v0::Message::try_compile(payer, &instructions, &[], blockhash)
            .map_err(|e| TxError::Invalid(format!("could not compile message: {e}")))?;

        let mut tx = VersionedTransaction {
            signatures: Vec::new(),
            message: VersionedMessage::V0(message),
        };

        // Pad with zeroed signatures so the tx is wire-serializable and the
        // frontend can deserialize + sign it. These placeholders are standard.
        let n_signers = tx.message.header().num_required_signatures.max(1) as usize; // payer is always a signer
        tx.signatures = vec![Signature::default(); n_signers];

        // Size guard: signatures + message must fit one network packet.
        let size = self.encoded_size(&tx);
        if size > MAX_TX_SIZE {
            return Err(TxError::TooLarge { size });
        }
        Ok(tx)
    }

    /// Approximate serialized size: 64 bytes per signature + message bytes.
    fn encoded_size(&self, tx: &VersionedTransaction) -> usize {
        1 + tx.signatures.len() * 64 + tx.message.serialize().len()
    }

    // -- safety validation (brief §13) ------------------------------------------

    /// Full funds-protection check. Runs on every unsigned transaction before
    /// it is ever shown to a user. Rejects:
    /// - any instruction from a program outside the allowlist
    /// - a transaction that does not make `payer` the first signer
    /// - a missing/duplicated/wrong-amount/wrong-destination fee transfer
    ///   (when a fee is expected)
    pub fn validate(
        &self,
        tx: &VersionedTransaction,
        payer: &Pubkey,
        expected_fee_base: Option<i64>,
    ) -> Result<(), TxError> {
        let allowlist: Vec<Pubkey> = {
            let ids = [
                Pubkey::from_str(TOKEN_PROGRAM).unwrap(),
                Pubkey::from_str(ATA_PROGRAM).unwrap(),
                Pubkey::from_str(COMPUTE_BUDGET_PROGRAM).unwrap(),
                Pubkey::from_str(SYSTEM_PROGRAM).unwrap(),
            ];
            let mut all: Vec<Pubkey> = ids.to_vec();
            all.extend(self.panta_programs.iter().copied());
            all
        };

        // Static account list of the compiled message (indexing into it is
        // how instructions reference accounts).
        let static_keys: Vec<Pubkey> = match &tx.message {
            VersionedMessage::V0(m) => m.account_keys.clone(),
            VersionedMessage::Legacy(m) => m.account_keys.clone(),
        };

        // 1. The payer must be the first signer — the user pays, never us.
        let header = tx.message.header();
        if header.num_required_signatures == 0 || static_keys.first() != Some(payer) {
            return Err(TxError::Validation(
                "payer must be the first signer of the transaction".into(),
            ));
        }

        // 2. Every instruction must come from an allowlisted program, and all
        //    its accounts must resolve within the static key list (we never
        //    ship ALTs; an out-of-range index means something is wrong).
        let mut fee_transfers = Vec::new(); // (source, dest, amount) for fee checking
        let token_program = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
        for (i, compiled) in tx.message.instructions().iter().enumerate() {
            // Program id: index into the static keys; fail on any mismatch.
            let program_index = compiled.program_id_index as usize;
            if program_index >= static_keys.len() {
                return Err(TxError::Validation(format!(
                    "instruction {i} references a non-static program account (ALT not expected)"
                )));
            }
            let program_id = &static_keys[program_index];
            if !allowlist.contains(program_id) {
                return Err(TxError::Validation(format!(
                    "instruction {i} uses unknown program {program_id}"
                )));
            }

            // Resolve account pubkeys by index (no lookup tables in MVP txs).
            let mut accounts = Vec::with_capacity(compiled.accounts.len());
            for idx in compiled.accounts.iter() {
                let idx = *idx as usize;
                if idx >= static_keys.len() {
                    return Err(TxError::Validation(format!(
                        "instruction {i} references a non-static account (ALT not expected)"
                    )));
                }
                accounts.push(static_keys[idx]);
            }

            // Track any SPL transfer_checked for the fee audit below.
            if program_id == &token_program {
                if let Some((source, dest, amount)) =
                    decode_transfer_checked(&compiled.data, &accounts)
                {
                    fee_transfers.push((source, dest, amount));
                }
            }
        }

        // 3. Fee audit (only when we charge one on this transaction).
        if let Some(expected) = expected_fee_base {
            let fee_ata = self.fee_usdc_ata();
            let user_ata = self.user_usdc_ata(payer);
            let to_fee_wallet: Vec<_> = fee_transfers
                .iter()
                .filter(|(_, dest, _)| *dest == fee_ata)
                .collect();
            match to_fee_wallet.len() {
                1 => {
                    let (source, _dest, amount) = to_fee_wallet[0];
                    if *source != user_ata {
                        return Err(TxError::Validation(
                            "fee must be funded from the buyer's own USDC account".into(),
                        ));
                    }
                    if *amount != expected as u64 {
                        return Err(TxError::Validation(format!(
                            "fee amount {amount} does not match quoted fee {expected}"
                        )));
                    }
                }
                n => {
                    return Err(TxError::Validation(format!(
                        "expected exactly 1 fee transfer to the fee wallet, found {n}"
                    )));
                }
            }
        }

        Ok(())
    }

    // -- simulation / broadcast / confirm --------------------------------------

    /// Simulate an unsigned (zeroed-signature) transaction on our RPC.
    /// Skipped in mock mode (mock instructions do not exist on-chain).
    pub async fn simulate(&self, tx: &VersionedTransaction) -> Result<(), TxError> {
        if self.mock {
            tracing::warn!("mock mode: skipping on-chain simulation");
            return Ok(());
        }
        let result = self
            .rpc
            .simulate_transaction(tx)
            .await
            .map_err(|e| TxError::Rpc(format!("simulation call failed: {e}")))?
            .value; // solana-client wraps results in Response { value, .. }
        if let Some(err) = result.err {
            return Err(TxError::Simulation(format!(
                "{err}; logs: {:?}",
                result.logs.unwrap_or_default()
            )));
        }
        Ok(())
    }

    /// Broadcast a signed transaction and wait until it is confirmed.
    /// Returns the transaction signature. Retries the send until the
    /// blockhash would expire; confirms by polling signature status.
    pub async fn broadcast_and_confirm(
        &self,
        tx: &VersionedTransaction,
    ) -> Result<String, TxError> {
        if self.mock {
            tracing::warn!("mock mode: faking broadcast and confirmation");
            return Ok(mock_signature());
        }

        // Send with retry — preflight is enabled (skip_preflight=false).
        let mut last_err = None;
        for _ in 0..3 {
            match self.rpc.send_transaction(tx).await {
                Ok(sig) => {
                    // Confirmed-failed txs are terminal (not retried).
                    self.confirm(&sig).await?;
                    return Ok(sig.to_string());
                }
                Err(e) => {
                    tracing::warn!(error = %e, "broadcast failed; retrying");
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
        Err(TxError::Rpc(format!(
            "broadcast failed after retries: {}",
            last_err.map(|e| e.to_string()).unwrap_or_default()
        )))
    }

    /// Poll the signature status until confirmed or timed out (~90 s).
    async fn confirm(&self, sig: &Signature) -> Result<(), TxError> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
        while tokio::time::Instant::now() < deadline {
            let statuses = self
                .rpc
                .get_signature_statuses(&[*sig])
                .await
                .map_err(|e| TxError::Rpc(format!("status check failed: {e}")))?
                .value; // unwrap the Response envelope
            match statuses.first().and_then(|s| s.to_owned()) {
                Some(status) => {
                    if let Some(err) = status.err {
                        return Err(TxError::Simulation(format!("transaction failed: {err}")));
                    }
                    // `confirmations` is Some until finalized; None = finalized.
                    if status.confirmation_status.is_some() {
                        return Ok(()); // confirmed (or better)
                    }
                }
                None => { /* not seen yet */ }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        Err(TxError::NotConfirmed)
    }

    /// Cheap liveness probe for /readyz: can we reach the RPC at all?
    pub async fn health(&self) -> bool {
        if self.mock {
            return true;
        }
        self.rpc.get_latest_blockhash().await.is_ok()
    }
}

/// Decode an SPL `transfer_checked` instruction: (source, destination, amount).
/// Any other Token instruction shape returns None (ignored by the fee audit).
fn decode_transfer_checked(data: &[u8], accounts: &[Pubkey]) -> Option<(Pubkey, Pubkey, u64)> {
    if data.first() != Some(&SPL_TRANSFER_CHECKED) || data.len() != 10 || accounts.len() < 3 {
        return None;
    }
    let mut amount_bytes = [0u8; 8];
    amount_bytes.copy_from_slice(&data[1..9]);
    Some((accounts[0], accounts[2], u64::from_le_bytes(amount_bytes)))
}

/// Deterministic fake signature for mock/dev mode.
pub fn mock_signature() -> String {
    let sig = [7u8; 64]; // recognizable, obviously fake
    bs58::encode(sig).into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use solana_sdk::hash::Hash;
    use solana_sdk::pubkey::Pubkey;

    /// Build a SolanaCtx for tests: mock mode (no RPC calls needed).
    fn ctx() -> SolanaCtx {
        SolanaCtx {
            rpc: RpcClient::new("http://localhost:8899".to_string()),
            usdc_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            fee_wallet: Pubkey::from_str("1FeeWa11et111111111111111111111111111111111").unwrap(),
            panta_programs: vec![
                Pubkey::from_str("1Panta1111111111111111111111111111111111111").unwrap(),
            ],
            priority_fee_microlamports: 5000,
            mock: true,
        }
    }

    fn zero_hash() -> Hash {
        Hash::default()
    }

    /// One Panta-shaped leg instruction against a fake market account.
    fn leg_instruction(user: Pubkey) -> Vec<Instruction> {
        vec![Instruction {
            program_id: Pubkey::from_str("1Panta1111111111111111111111111111111111111").unwrap(),
            accounts: vec![
                AccountMeta::new_readonly(user, true),
                AccountMeta::new(Pubkey::new_unique(), false),
            ],
            data: vec![9, 9, 9],
        }]
    }

    #[test]
    fn fee_instruction_shape() {
        let ctx = ctx();
        let user = Pubkey::new_unique();
        let ix = ctx.fee_instruction(&user, 150_000).unwrap(); // 0.15 USDC

        assert_eq!(ix.program_id.to_string(), TOKEN_PROGRAM);
        assert_eq!(ix.accounts.len(), 4);
        assert_eq!(ix.accounts[0].pubkey, ctx.user_usdc_ata(&user)); // source
        assert_eq!(ix.accounts[1].pubkey, ctx.usdc_mint); // mint
        assert_eq!(ix.accounts[2].pubkey, ctx.fee_usdc_ata()); // fee destination
        assert!(ix.accounts[3].is_signer); // buyer authorizes
        assert_eq!(
            ix.data,
            [vec![12u8], 150_000u64.to_le_bytes().to_vec(), vec![6u8]].concat()
        );
    }

    #[test]
    fn negative_fee_rejected() {
        let ctx = ctx();
        assert!(ctx.fee_instruction(&Pubkey::new_unique(), -1).is_err());
    }

    #[test]
    fn parse_wire_instruction_round_trip() {
        let wire = WireInstruction {
            program_id: TOKEN_PROGRAM.to_string(),
            data: base64::engine::general_purpose::STANDARD.encode(vec![1, 2, 3]),
            accounts: vec![crate::panta::WireAccount {
                pubkey: "1FeeWa11et111111111111111111111111111111111".to_string(),
                is_signer: true,
                is_writable: false,
            }],
        };
        let ix = SolanaCtx::parse_wire_instruction(&wire).unwrap();
        assert_eq!(ix.data, vec![1, 2, 3]);
        assert!(ix.accounts[0].is_signer && !ix.accounts[0].is_writable);

        // Garbage base64 must fail, not panic.
        let bad = WireInstruction {
            data: "!!!".into(),
            ..wire.clone()
        };
        assert!(SolanaCtx::parse_wire_instruction(&bad).is_err());

        // A program id that is not a valid pubkey must fail too.
        let bad_program = WireInstruction {
            program_id: "not a pubkey".into(),
            ..wire.clone()
        };
        assert!(SolanaCtx::parse_wire_instruction(&bad_program).is_err());
    }

    #[test]
    fn assemble_places_payer_first_and_fits() {
        let ctx = ctx();
        let user = Pubkey::new_unique();
        let tx = ctx
            .assemble_with_blockhash(
                &user,
                vec![leg_instruction(user), leg_instruction(user)],
                Some(75_000),
                400_000,
                zero_hash(),
            )
            .unwrap();

        // Payer is the first static account (first signer).
        assert_eq!(tx.message.static_account_keys()[0], user);
        // Instruction order: 2 compute budget + 2 legs + 1 fee = 5.
        assert_eq!(tx.message.instructions().len(), 5);
        // All signatures zeroed for the wallet to fill.
        assert!(tx.signatures.iter().all(|s| s == &Signature::default()));
        // v0 message, single required signer.
        assert!(matches!(tx.message, VersionedMessage::V0(_)));
    }

    #[test]
    fn validate_accepts_wellformed_purchase() {
        let ctx = ctx();
        let user = Pubkey::new_unique();
        let tx = ctx
            .assemble_with_blockhash(
                &user,
                vec![leg_instruction(user)],
                Some(75_000),
                400_000,
                zero_hash(),
            )
            .unwrap();
        assert!(ctx.validate(&tx, &user, Some(75_000)).is_ok());
    }

    #[test]
    fn validate_rejects_foreign_program() {
        let ctx = ctx();
        let user = Pubkey::new_unique();
        let mut rogue = leg_instruction(user);
        rogue[0].program_id = Pubkey::new_unique(); // not on the allowlist
        let tx = ctx
            .assemble_with_blockhash(&user, vec![rogue], Some(75_000), 400_000, zero_hash())
            .unwrap();
        let err = ctx.validate(&tx, &user, Some(75_000)).unwrap_err();
        assert!(matches!(err, TxError::Validation(_)));
    }

    #[test]
    fn validate_rejects_wrong_fee_amount_or_destination() {
        let ctx = ctx();
        let user = Pubkey::new_unique();
        let tx = ctx
            .assemble_with_blockhash(
                &user,
                vec![leg_instruction(user)],
                Some(75_000),
                400_000,
                zero_hash(),
            )
            .unwrap();

        // Quoted 100_000 but tx pays 75_000 -> reject.
        assert!(ctx.validate(&tx, &user, Some(100_000)).is_err());

        // No fee transfer at all when one is expected -> reject.
        let no_fee = ctx
            .assemble_with_blockhash(
                &user,
                vec![leg_instruction(user)],
                None,
                400_000,
                zero_hash(),
            )
            .unwrap();
        assert!(ctx.validate(&no_fee, &user, Some(75_000)).is_err());
    }

    #[test]
    fn validate_rejects_wrong_payer() {
        let ctx = ctx();
        let user = Pubkey::new_unique();
        let tx = ctx
            .assemble_with_blockhash(
                &user,
                vec![leg_instruction(user)],
                Some(75_000),
                400_000,
                zero_hash(),
            )
            .unwrap();
        // Claim a different payer than the one who signed -> reject.
        assert!(ctx
            .validate(&tx, &Pubkey::new_unique(), Some(75_000))
            .is_err());
    }

    #[test]
    fn decode_transfer_checked_rejects_other_shapes() {
        let a = Pubkey::new_unique();
        let b = Pubkey::new_unique();
        let c = Pubkey::new_unique();
        assert!(decode_transfer_checked(&[12, 0, 0], &[a, b, c]).is_none()); // too short
        assert!(decode_transfer_checked(&[3, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[a, b, c]).is_none()); // wrong tag
        let good = decode_transfer_checked(&[12, 1, 0, 0, 0, 0, 0, 0, 0, 6], &[a, b, c]).unwrap();
        assert_eq!(good.2, 1);
    }

    /// Two legs + fee must stay under the packet limit (MVP sizing guarantee).
    #[test]
    fn four_legs_plus_fee_fit_in_one_tx() {
        let ctx = ctx();
        let user = Pubkey::new_unique();
        let legs: Vec<Vec<Instruction>> = (0..4).map(|_| leg_instruction(user)).collect();
        let tx = ctx
            .assemble_with_blockhash(&user, legs, Some(75_000), 400_000, zero_hash())
            .unwrap();
        assert!(ctx.encoded_size(&tx) <= MAX_TX_SIZE);
    }

    // Config plumbing sanity: the new fields exist with documented defaults.
    #[test]
    fn config_fields_exist() {
        let c = base_test_config();
        assert!(c.panta_program_ids.is_empty()); // empty by default (dev)
        assert_eq!(c.priority_fee_microlamports, 5000); // documented default
    }

    fn base_test_config() -> Config {
        // Minimal valid config for assertions on defaults.
        Config {
            app_env: "development".into(),
            database_url: "postgres://x".into(),
            panta_api_base_url: "https://example.test".into(),
            panta_api_key: String::new(),
            panta_mock: true,
            solana_rpc_url: "http://localhost:8899".into(),
            solana_ws_url: None,
            usdc_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
            fee_wallet_pubkey: String::new(),
            app_fee_bps: 75,
            max_basket_legs: 4,
            default_slippage_bps: 150,
            jwt_secret: "test".into(),
            admin_wallets: vec![],
            blocked_countries: vec![],
            cors_origins: vec![],
            api_bind: "0.0.0.0:8080".into(),
            panta_program_ids: vec![],
            priority_fee_microlamports: 5000,
        }
    }
}
