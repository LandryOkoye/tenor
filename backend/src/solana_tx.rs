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
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
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
/// SPL Memo v2. Panta's purchase builds append a memo; memos cannot move funds.
pub const MEMO_PROGRAM: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
/// System program (mock instructions in dev mode; never expected in real buys).
pub const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

/// SPL Token `transfer_checked` instruction discriminant.
const SPL_TRANSFER_CHECKED: u8 = 12;
/// ComputeBudget instruction tags (solana-compute-budget-interface).
const CB_SET_COMPUTE_UNIT_LIMIT: u8 = 2;
const CB_SET_COMPUTE_UNIT_PRICE: u8 = 3;
/// Runtime ceiling on compute units per transaction.
const MAX_CU_LIMIT: u32 = 1_400_000;
/// ATA program instructions we allow: Create (0) and CreateIdempotent (1).
/// Legacy Create is encoded with empty data.
const ATA_ALLOWED_TAGS: [u8; 2] = [0, 1];
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

/// An unsigned transaction plus the block height after which its blockhash
/// expires (used by the confirmation job to declare a tx dead).
pub struct AssembledTx {
    pub tx: VersionedTransaction,
    pub last_valid_block_height: u64,
}

impl AssembledTx {
    /// Wire bytes of the message — stored at build time and compared
    /// byte-for-byte against what the user signs (see `check_signed`).
    pub fn message_bytes(&self) -> Vec<u8> {
        self.tx.message.serialize()
    }

    /// Blockhash expiry as a DB value (None in mock mode, where it never expires).
    pub fn expiry(&self) -> Option<i64> {
        i64::try_from(self.last_valid_block_height).ok()
    }
}

/// On-chain state of a signature, as the confirmation logic needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigStatus {
    /// Not seen by the cluster (yet).
    Unknown,
    /// Seen at `processed` only — can still be dropped by a fork.
    Processed,
    /// `confirmed` or `finalized` and succeeded.
    Confirmed,
    /// Landed but the program returned an error.
    Failed(String),
}

// --- Context ----------------------------------------------------------------

/// Everything the transaction layer needs, built once at startup.
pub struct SolanaCtx {
    rpc: RpcClient,
    /// Same endpoint as `rpc`, for raw JSON-RPC calls the client lacks.
    rpc_url: String,
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
        // Our own basket_vault program is allowed at the top level too.
        for id in config
            .panta_program_ids
            .iter()
            .chain(&config.vault_program_id)
        {
            panta_programs.push(
                Pubkey::from_str(id)
                    .map_err(|e| anyhow::anyhow!("PANTA_PROGRAM_IDS entry {id:?} invalid: {e}"))?,
            );
        }

        Ok(Self {
            rpc: RpcClient::new(config.solana_rpc_url.clone()),
            rpc_url: config.solana_rpc_url.clone(),
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

    /// ComputeBudget instructions: a CU limit and a priority fee. Built with
    /// the SDK helpers so the instruction tags always match the runtime
    /// (2 = SetComputeUnitLimit, 3 = SetComputeUnitPrice).
    fn compute_budget_instructions(&self, cu_limit: u32) -> Vec<Instruction> {
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(cu_limit),
            ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee_microlamports),
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
    /// fills them in when signing. Fetches a fresh blockhash from our RPC
    /// (mock mode uses a zero blockhash so dev works offline).
    pub async fn assemble(
        &self,
        payer: &Pubkey,
        leg_instructions: Vec<Vec<Instruction>>,
        fee_base: Option<i64>,
        cu_limit: u32,
    ) -> Result<AssembledTx, TxError> {
        let (blockhash, last_valid_block_height) = if self.mock {
            // Random per build so two mock purchases never share a signature
            // (even across restarts).
            (
                solana_sdk::hash::hashv(&[uuid::Uuid::new_v4().as_bytes()]),
                u64::MAX,
            )
        } else {
            self.rpc
                .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
                .await
                .map_err(|e| TxError::Rpc(format!("could not get blockhash: {e}")))?
        };
        let tx =
            self.assemble_with_blockhash(payer, leg_instructions, fee_base, cu_limit, blockhash)?;
        Ok(AssembledTx {
            tx,
            last_valid_block_height,
        })
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
        // Any ComputeBudget instructions inside the legs (e.g. sent by Panta)
        // are dropped: we set exactly one limit + one price ourselves, and
        // duplicates make the runtime reject the transaction.
        let compute_budget = Pubkey::from_str(COMPUTE_BUDGET_PROGRAM).expect("compute budget id");
        let mut instructions = self.compute_budget_instructions(cu_limit);
        for leg in leg_instructions {
            instructions.extend(leg.into_iter().filter(|ix| ix.program_id != compute_budget));
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
        let token_program = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
        let ata_program = Pubkey::from_str(ATA_PROGRAM).unwrap();
        let compute_budget_program = Pubkey::from_str(COMPUTE_BUDGET_PROGRAM).unwrap();
        let system_program = Pubkey::from_str(SYSTEM_PROGRAM).unwrap();

        // The System program is only ever used by mock instructions; a real
        // purchase never needs a top-level System instruction, and allowing
        // one would let a SOL transfer to anyone through.
        let memo_program = Pubkey::from_str(MEMO_PROGRAM).unwrap();
        let mut allowlist = vec![
            token_program,
            ata_program,
            compute_budget_program,
            memo_program,
        ];
        if self.mock {
            allowlist.push(system_program);
        }
        allowlist.extend(self.panta_programs.iter().copied());

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
        let (mut cu_limit_seen, mut cu_price_seen) = (false, false);
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

            let data = compiled.data.as_slice();

            // Token program: the ONLY token instruction we ever allow at the
            // top level is a transfer_checked (our fee, audited below). Any
            // other shape (transfer, approve, set_authority, close, …) could
            // move or expose the user's funds.
            if program_id == &token_program {
                match decode_transfer_checked(data, &accounts) {
                    Some(transfer) => fee_transfers.push(transfer),
                    None => {
                        return Err(TxError::Validation(format!(
                            "instruction {i}: only a fee transfer_checked is allowed on the token program"
                        )))
                    }
                }
            }

            // ATA program: account creation only (no RecoverNested).
            if program_id == &ata_program
                && !(data.is_empty() || (data.len() == 1 && ATA_ALLOWED_TAGS.contains(&data[0])))
            {
                return Err(TxError::Validation(format!(
                    "instruction {i}: only associated-account creation is allowed"
                )));
            }

            // ComputeBudget: exactly our CU limit + our priority price, once
            // each. A foreign price instruction could drain the payer's SOL.
            if program_id == &compute_budget_program {
                self.check_compute_budget(i, data, &mut cu_limit_seen, &mut cu_price_seen)?;
            }
        }

        // 2b. Every transfer_checked must be our fee: user's ATA -> fee ATA.
        if expected_fee_base.is_none() && !fee_transfers.is_empty() {
            return Err(TxError::Validation(
                "unexpected token transfer in a transaction that carries no fee".into(),
            ));
        }
        let fee_ata = self.fee_usdc_ata();
        if let Some((_, dest, _)) = fee_transfers.iter().find(|(_, d, _)| *d != fee_ata) {
            return Err(TxError::Validation(format!(
                "token transfer to {dest}, which is not the fee wallet"
            )));
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

    /// Simulate and return the post-transaction token `amount` of one SPL
    /// token account (e.g. the basket tokens a deposit would mint).
    pub async fn simulate_token_balance(
        &self,
        tx: &VersionedTransaction,
        token_account: &Pubkey,
    ) -> Result<u64, TxError> {
        use solana_client::rpc_config::{
            RpcSimulateTransactionAccountsConfig, RpcSimulateTransactionConfig,
        };
        let result = self
            .rpc
            .simulate_transaction_with_config(
                tx,
                RpcSimulateTransactionConfig {
                    sig_verify: false,
                    replace_recent_blockhash: true,
                    accounts: Some(RpcSimulateTransactionAccountsConfig {
                        encoding: None, // base64
                        addresses: vec![token_account.to_string()],
                    }),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| TxError::Rpc(format!("simulation call failed: {e}")))?
            .value;
        if let Some(err) = result.err {
            return Err(TxError::Simulation(format!(
                "{err}; logs: {:?}",
                result.logs.unwrap_or_default()
            )));
        }
        let data = result
            .accounts
            .and_then(|a| a.into_iter().next().flatten())
            .and_then(|acc| acc.data.decode())
            .ok_or_else(|| TxError::Simulation("simulated account missing".into()))?;
        // SPL token account: amount is the u64 at offset 64.
        data.get(64..72)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| TxError::Simulation("not a token account".into()))
    }

    /// Raw JSON-RPC call on our endpoint (for methods we decode by hand).
    pub async fn rpc_json(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, TxError> {
        let body =
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let resp: serde_json::Value = reqwest::Client::new()
            .post(&self.rpc_url)
            .json(&body)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|e| TxError::Rpc(format!("{method}: {e}")))?
            .json()
            .await
            .map_err(|e| TxError::Rpc(format!("{method}: {e}")))?;
        if let Some(err) = resp.get("error") {
            return Err(TxError::Rpc(format!("{method}: {err}")));
        }
        Ok(resp["result"].clone())
    }

    /// Raw account data for several accounts (None where missing).
    pub async fn accounts_data(&self, keys: &[Pubkey]) -> Result<Vec<Option<Vec<u8>>>, TxError> {
        let accounts = self
            .rpc
            .get_multiple_accounts(keys)
            .await
            .map_err(|e| TxError::Rpc(format!("get accounts failed: {e}")))?;
        Ok(accounts.into_iter().map(|a| a.map(|a| a.data)).collect())
    }

    pub fn is_mock(&self) -> bool {
        self.mock
    }

    /// The signature a signed transaction will land under (its first
    /// signature, i.e. the fee payer's). Known BEFORE broadcasting, so we can
    /// persist it first and never lose track of a transaction.
    pub fn signature_of(tx: &VersionedTransaction) -> Result<String, TxError> {
        tx.signatures
            .first()
            .filter(|s| **s != Signature::default())
            .map(|s| s.to_string())
            .ok_or_else(|| TxError::Validation("transaction is not signed".into()))
    }

    /// Broadcast a signed transaction (preflight on). Retries transport
    /// errors a few times; re-sending the same signed tx is harmless.
    /// Returns the signature. Mock mode sends nothing.
    pub async fn send(&self, tx: &VersionedTransaction) -> Result<String, TxError> {
        let signature = Self::signature_of(tx)?;
        if self.mock {
            tracing::warn!("mock mode: not broadcasting");
            return Ok(signature);
        }
        let mut last_err = None;
        for _ in 0..3 {
            match self.rpc.send_transaction(tx).await {
                Ok(_) => return Ok(signature),
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

    /// Current on-chain status of a signature. Only `confirmed`/`finalized`
    /// count as landed — `processed` can still be rolled back.
    pub async fn signature_status(&self, signature: &str) -> Result<SigStatus, TxError> {
        if self.mock {
            return Ok(SigStatus::Confirmed);
        }
        let sig = Signature::from_str(signature)
            .map_err(|e| TxError::Invalid(format!("bad signature {signature:?}: {e}")))?;
        let statuses = self
            .rpc
            .get_signature_statuses(&[sig])
            .await
            .map_err(|e| TxError::Rpc(format!("status check failed: {e}")))?
            .value;
        Ok(match statuses.into_iter().next().flatten() {
            None => SigStatus::Unknown,
            Some(s) => match &s.err {
                Some(err) => SigStatus::Failed(err.to_string()),
                None if s.satisfies_commitment(CommitmentConfig::confirmed()) => {
                    SigStatus::Confirmed
                }
                None => SigStatus::Processed,
            },
        })
    }

    /// Poll a signature for up to `timeout`, returning as soon as it is
    /// confirmed or failed (otherwise the last status seen). Callers hand any
    /// still-pending signature to the `tx_confirm` worker job.
    pub async fn wait_for(&self, signature: &str, timeout: Duration) -> SigStatus {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last = SigStatus::Unknown;
        loop {
            match self.signature_status(signature).await {
                Ok(s @ (SigStatus::Confirmed | SigStatus::Failed(_))) => return s,
                Ok(s) => last = s,
                Err(e) => tracing::warn!(error = %e, "signature status check failed"),
            }
            if tokio::time::Instant::now() >= deadline {
                return last;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Current block height (confirmed). Used to decide that an unseen
    /// signature's blockhash has expired, so the tx can never land.
    pub async fn block_height(&self) -> Result<u64, TxError> {
        if self.mock {
            return Ok(0);
        }
        self.rpc
            .get_block_height_with_commitment(CommitmentConfig::confirmed())
            .await
            .map_err(|e| TxError::Rpc(format!("block height failed: {e}")))
    }

    /// Check that a transaction coming back from the user's wallet is the
    /// exact one we built and validated: identical message bytes (so no
    /// instruction, account, amount or fee was changed or dropped) and a
    /// valid signature from every required signer.
    pub fn check_signed(
        signed: &VersionedTransaction,
        expected_message: &[u8],
    ) -> Result<(), TxError> {
        if signed.message.serialize() != expected_message {
            return Err(TxError::Validation(
                "signed transaction does not match the transaction we built".into(),
            ));
        }
        let required = signed.message.header().num_required_signatures as usize;
        let results = signed.verify_with_results();
        if results.len() != required || !results.iter().all(|ok| *ok) {
            return Err(TxError::Validation(
                "transaction signatures are missing or invalid".into(),
            ));
        }
        Ok(())
    }

    /// ComputeBudget audit: at most one CU limit (within the runtime cap) and
    /// at most one CU price equal to our configured priority fee. Anything
    /// else (heap frames, foreign prices) is rejected.
    fn check_compute_budget(
        &self,
        i: usize,
        data: &[u8],
        limit_seen: &mut bool,
        price_seen: &mut bool,
    ) -> Result<(), TxError> {
        let reject = |why: &str| Err(TxError::Validation(format!("instruction {i}: {why}")));
        match data.first() {
            Some(&CB_SET_COMPUTE_UNIT_LIMIT) if data.len() == 5 && !*limit_seen => {
                let limit = u32::from_le_bytes(data[1..5].try_into().unwrap());
                if limit > MAX_CU_LIMIT {
                    return reject("compute unit limit above runtime maximum");
                }
                *limit_seen = true;
                Ok(())
            }
            Some(&CB_SET_COMPUTE_UNIT_PRICE) if data.len() == 9 && !*price_seen => {
                let price = u64::from_le_bytes(data[1..9].try_into().unwrap());
                if price != self.priority_fee_microlamports {
                    return reject("unexpected priority fee");
                }
                *price_seen = true;
                Ok(())
            }
            _ => reject("unexpected compute budget instruction"),
        }
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
            rpc_url: "http://localhost:8899".to_string(),
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

    /// Production-mode context (System program NOT allowed).
    fn real_ctx() -> SolanaCtx {
        SolanaCtx {
            mock: false,
            ..ctx()
        }
    }

    fn purchase_tx(ctx: &SolanaCtx, user: Pubkey, extra: Vec<Instruction>) -> VersionedTransaction {
        let mut legs = vec![leg_instruction(user)];
        legs.push(extra);
        ctx.assemble_with_blockhash(&user, legs, Some(75_000), 400_000, zero_hash())
            .unwrap()
    }

    #[test]
    fn compute_budget_uses_runtime_tags() {
        let ixs = ctx().compute_budget_instructions(400_000);
        assert_eq!(ixs[0].data[0], CB_SET_COMPUTE_UNIT_LIMIT);
        assert_eq!(ixs[0].data[1..], 400_000u32.to_le_bytes());
        assert_eq!(ixs[1].data[0], CB_SET_COMPUTE_UNIT_PRICE);
        assert_eq!(ixs[1].data[1..], 5_000u64.to_le_bytes());
    }

    #[test]
    fn validate_rejects_system_program_outside_mock() {
        let ctx = real_ctx();
        let user = Pubkey::new_unique();
        let drain = Instruction {
            program_id: Pubkey::from_str(SYSTEM_PROGRAM).unwrap(),
            accounts: vec![
                AccountMeta::new(user, true),
                AccountMeta::new(Pubkey::new_unique(), false),
            ],
            data: vec![2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0], // system transfer
        };
        let tx = purchase_tx(&ctx, user, vec![drain]);
        assert!(ctx.validate(&tx, &user, Some(75_000)).is_err());
    }

    #[test]
    fn validate_rejects_extra_token_instructions() {
        let ctx = real_ctx();
        let user = Pubkey::new_unique();
        let token = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
        let attacker = Pubkey::new_unique();

        // An `approve` (tag 4) delegating the user's USDC to someone else.
        let approve = Instruction {
            program_id: token,
            accounts: vec![
                AccountMeta::new(ctx.user_usdc_ata(&user), false),
                AccountMeta::new_readonly(attacker, false),
                AccountMeta::new_readonly(user, true),
            ],
            data: [vec![4u8], 1_000_000u64.to_le_bytes().to_vec()].concat(),
        };
        let tx = purchase_tx(&ctx, user, vec![approve]);
        assert!(ctx.validate(&tx, &user, Some(75_000)).is_err());

        // A second transfer_checked to a wallet that is not ours.
        let mut steal = ctx.fee_instruction(&user, 75_000).unwrap();
        steal.accounts[2].pubkey = attacker;
        let tx = purchase_tx(&ctx, user, vec![steal]);
        assert!(ctx.validate(&tx, &user, Some(75_000)).is_err());
    }

    #[test]
    fn validate_rejects_foreign_priority_fee() {
        let ctx = real_ctx();
        let user = Pubkey::new_unique();
        // Assembly strips leg compute-budget instructions...
        let leg = vec![ComputeBudgetInstruction::set_compute_unit_price(
            u64::MAX / 2,
        )];
        let tx = ctx
            .assemble_with_blockhash(&user, vec![leg], Some(75_000), 400_000, zero_hash())
            .unwrap();
        assert!(ctx.validate(&tx, &user, Some(75_000)).is_ok());

        // ...and validation independently rejects a tx that carries a huge
        // foreign priority fee (would drain SOL as fees).
        let ixs = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(400_000),
            ComputeBudgetInstruction::set_compute_unit_price(u64::MAX / 2),
            ctx.fee_instruction(&user, 75_000).unwrap(),
        ];
        let message = v0::Message::try_compile(&user, &ixs, &[], zero_hash()).unwrap();
        let tx = VersionedTransaction {
            signatures: vec![Signature::default()],
            message: VersionedMessage::V0(message),
        };
        assert!(ctx.validate(&tx, &user, Some(75_000)).is_err());
    }

    #[test]
    fn validate_accepts_real_mode_purchase() {
        let ctx = real_ctx();
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
    fn check_signed_requires_identical_message_and_valid_signature() {
        use solana_sdk::signer::{keypair::Keypair, Signer};
        let ctx = real_ctx();
        let kp = Keypair::new();
        let user = kp.pubkey();
        let built = ctx
            .assemble_with_blockhash(
                &user,
                vec![leg_instruction(user)],
                Some(75_000),
                400_000,
                zero_hash(),
            )
            .unwrap();
        let built_bytes = built.message.serialize();

        // Unsigned (zeroed signature) -> rejected.
        assert!(SolanaCtx::check_signed(&built, &built_bytes).is_err());

        // Properly signed -> accepted, and the signature is known up front.
        let signed = VersionedTransaction::try_new(built.message.clone(), &[&kp]).unwrap();
        assert!(SolanaCtx::check_signed(&signed, &built_bytes).is_ok());
        assert!(SolanaCtx::signature_of(&signed).is_ok());

        // A different tx (fee instruction dropped), even validly signed -> rejected.
        let no_fee = ctx
            .assemble_with_blockhash(
                &user,
                vec![leg_instruction(user)],
                None,
                400_000,
                zero_hash(),
            )
            .unwrap();
        let no_fee_signed = VersionedTransaction::try_new(no_fee.message, &[&kp]).unwrap();
        assert!(SolanaCtx::check_signed(&no_fee_signed, &built_bytes).is_err());

        // Right message, wrong signer -> rejected.
        let other = Keypair::new();
        let mut forged = signed.clone();
        forged.signatures[0] = other.sign_message(&built_bytes);
        assert!(SolanaCtx::check_signed(&forged, &built_bytes).is_err());
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
            vault_program_id: None,
            crank_keypair: None,
            priority_fee_microlamports: 5000,
        }
    }
}
