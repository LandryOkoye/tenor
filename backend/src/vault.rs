//! Off-chain side of the `basket_vault` program (tokenized baskets, ADR 0002):
//! PDA derivation, instruction builders, Panta market-account discovery, and
//! on-chain NAV reads. Layouts mirror `onchain/programs/basket_vault` and
//! docs/panta-notes.md.

use std::str::FromStr;

use solana_sdk::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::solana_tx::{SolanaCtx, TxError, ATA_PROGRAM, SYSTEM_PROGRAM, TOKEN_PROGRAM};

/// Panta prediction-market program (mainnet).
pub const PANTA_PROGRAM: &str = "6gM5afTQBq5VZCfgpGqcsqzfWd5maLSCKWtGjbEobZMp";
/// `primary_order_usdc` discriminator — used to spot buys in market history.
const PRIMARY_ORDER_USDC: [u8; 8] = [0x2e, 0x89, 0x44, 0x74, 0x31, 0x59, 0x0d, 0xf7];
/// Panta `Event.yes_price` offset (u64, 1e9 = 1 USDC).
const EVENT_YES_PRICE: usize = 152;
/// Panta `UserPosition` share offsets.
const POSITION_YES_SHARES: usize = 74;
const POSITION_NO_SHARES: usize = 90;
/// Panta price scale.
const PRICE_ONE: u128 = 1_000_000_000;

/// Accounts of one leg, as the vault program expects them.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultLeg {
    pub market_id: String,
    pub event: String,
    pub authority: String,
    pub market_vault: String,
    pub side: u8,
    pub weight_bps: u16,
}

/// Everything derived from a basket's on-chain id.
pub struct VaultAddresses {
    pub program: Pubkey,
    pub basket: Pubkey,
    pub vault: Pubkey,
    pub mint: Pubkey,
    pub vault_usdc: Pubkey,
}

fn pk(s: &str) -> Result<Pubkey, TxError> {
    Pubkey::from_str(s).map_err(|e| TxError::Invalid(format!("bad pubkey {s:?}: {e}")))
}

/// Anchor instruction discriminator.
fn disc(name: &str) -> Vec<u8> {
    hash(format!("global:{name}").as_bytes()).to_bytes()[..8].to_vec()
}

pub fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
    let ata = Pubkey::from_str(ATA_PROGRAM).unwrap();
    Pubkey::find_program_address(&[owner.as_ref(), token.as_ref(), mint.as_ref()], &ata).0
}

pub fn addresses(program: &Pubkey, onchain_id: u64, usdc: &Pubkey) -> VaultAddresses {
    let basket = Pubkey::find_program_address(&[b"basket", &onchain_id.to_le_bytes()], program).0;
    let vault = Pubkey::find_program_address(&[b"vault", basket.as_ref()], program).0;
    let mint = Pubkey::find_program_address(&[b"mint", basket.as_ref()], program).0;
    VaultAddresses {
        program: *program,
        basket,
        vault,
        mint,
        vault_usdc: ata(&vault, usdc),
    }
}

/// Panta's MarketConfig PDA: `["market_config"]`.
pub fn panta_config() -> Pubkey {
    Pubkey::find_program_address(
        &[b"market_config"],
        &Pubkey::from_str(PANTA_PROGRAM).unwrap(),
    )
    .0
}

/// The vault's Panta `UserPosition`: `["position", event, vault]`.
pub fn position(event: &Pubkey, owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"position", event.as_ref(), owner.as_ref()],
        &Pubkey::from_str(PANTA_PROGRAM).unwrap(),
    )
    .0
}

/// Idempotent ATA creation (payer funds it).
pub fn create_ata_idempotent(payer: &Pubkey, owner: &Pubkey, mint: &Pubkey) -> Instruction {
    Instruction {
        program_id: Pubkey::from_str(ATA_PROGRAM).unwrap(),
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(ata(owner, mint), false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(Pubkey::from_str(SYSTEM_PROGRAM).unwrap(), false),
            AccountMeta::new_readonly(Pubkey::from_str(TOKEN_PROGRAM).unwrap(), false),
        ],
        data: vec![1],
    }
}

/// `create_basket(id, legs, fee_bps)`, signed by the admin (who pays rent).
pub fn create_basket_ix(
    a: &VaultAddresses,
    admin: &Pubkey,
    onchain_id: u64,
    legs: &[VaultLeg],
    fee_bps: u16,
    fee_usdc: &Pubkey,
    usdc: &Pubkey,
) -> Result<Instruction, TxError> {
    let mut data = disc("create_basket");
    data.extend_from_slice(&onchain_id.to_le_bytes());
    data.extend_from_slice(&(legs.len() as u32).to_le_bytes());
    for leg in legs {
        data.extend_from_slice(pk(&leg.event)?.as_ref());
        data.push(leg.side);
        data.extend_from_slice(&leg.weight_bps.to_le_bytes());
    }
    data.extend_from_slice(&fee_bps.to_le_bytes());
    Ok(Instruction {
        program_id: a.program,
        accounts: vec![
            AccountMeta::new(*admin, true),
            AccountMeta::new(a.basket, false),
            AccountMeta::new(a.vault, false),
            AccountMeta::new(a.mint, false),
            AccountMeta::new(a.vault_usdc, false),
            AccountMeta::new_readonly(*fee_usdc, false),
            AccountMeta::new_readonly(*usdc, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM)?, false),
            AccountMeta::new_readonly(pk(ATA_PROGRAM)?, false),
            AccountMeta::new_readonly(pk(SYSTEM_PROGRAM)?, false),
        ],
        data,
    })
}

/// `deposit(amount, min_tokens_out)` for `user`.
#[allow(clippy::too_many_arguments)]
pub fn deposit_ix(
    a: &VaultAddresses,
    user: &Pubkey,
    legs: &[VaultLeg],
    fee_usdc: &Pubkey,
    panta_fee_usdc: &Pubkey,
    usdc: &Pubkey,
    amount: u64,
    min_tokens_out: u64,
) -> Result<Instruction, TxError> {
    let mut data = disc("deposit");
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&min_tokens_out.to_le_bytes());
    let mut accounts = vec![
        AccountMeta::new(*user, true),
        AccountMeta::new_readonly(a.basket, false),
        AccountMeta::new(a.vault, false),
        AccountMeta::new(a.mint, false),
        AccountMeta::new(a.vault_usdc, false),
        AccountMeta::new(ata(user, usdc), false),
        AccountMeta::new(ata(user, &a.mint), false),
        AccountMeta::new(*fee_usdc, false),
        AccountMeta::new_readonly(*usdc, false),
        AccountMeta::new_readonly(panta_config(), false),
        AccountMeta::new(*panta_fee_usdc, false),
        AccountMeta::new_readonly(pk(PANTA_PROGRAM)?, false),
        AccountMeta::new_readonly(pk(TOKEN_PROGRAM)?, false),
        AccountMeta::new_readonly(pk(ATA_PROGRAM)?, false),
        AccountMeta::new_readonly(pk(SYSTEM_PROGRAM)?, false),
    ];
    for leg in legs {
        let event = pk(&leg.event)?;
        accounts.push(AccountMeta::new(event, false));
        accounts.push(AccountMeta::new_readonly(pk(&leg.authority)?, false));
        accounts.push(AccountMeta::new(pk(&leg.market_vault)?, false));
        accounts.push(AccountMeta::new(position(&event, &a.vault), false));
    }
    Ok(Instruction {
        program_id: a.program,
        accounts,
        data,
    })
}

/// `redeem(tokens)` for `user` (basket must be settled).
pub fn redeem_ix(
    a: &VaultAddresses,
    user: &Pubkey,
    usdc: &Pubkey,
    tokens: u64,
) -> Result<Instruction, TxError> {
    let mut data = disc("redeem");
    data.extend_from_slice(&tokens.to_le_bytes());
    Ok(Instruction {
        program_id: a.program,
        accounts: vec![
            AccountMeta::new_readonly(*user, true),
            AccountMeta::new_readonly(a.basket, false),
            AccountMeta::new_readonly(a.vault, false),
            AccountMeta::new(a.mint, false),
            AccountMeta::new(a.vault_usdc, false),
            AccountMeta::new(ata(user, usdc), false),
            AccountMeta::new(ata(user, &a.mint), false),
            AccountMeta::new_readonly(*usdc, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM)?, false),
        ],
        data,
    })
}

/// The vault's Panta `WinClaim` receipt: `["win_claim", event, vault]`.
pub fn win_claim(event: &Pubkey, owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"win_claim", event.as_ref(), owner.as_ref()],
        &Pubkey::from_str(PANTA_PROGRAM).unwrap(),
    )
    .0
}

/// `claim_leg(leg_index)`: anyone may crank it; winnings go to the vault only.
pub fn claim_leg_ix(
    a: &VaultAddresses,
    leg_index: u8,
    leg: &VaultLeg,
    usdc: &Pubkey,
) -> Result<Instruction, TxError> {
    let mut data = disc("claim_leg");
    data.push(leg_index);
    let event = pk(&leg.event)?;
    Ok(Instruction {
        program_id: a.program,
        accounts: vec![
            AccountMeta::new_readonly(a.basket, false),
            AccountMeta::new(a.vault, false),
            AccountMeta::new(a.vault_usdc, false),
            AccountMeta::new_readonly(*usdc, false),
            AccountMeta::new_readonly(panta_config(), false),
            AccountMeta::new_readonly(pk(PANTA_PROGRAM)?, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM)?, false),
            AccountMeta::new_readonly(pk(ATA_PROGRAM)?, false),
            AccountMeta::new_readonly(pk(SYSTEM_PROGRAM)?, false),
            AccountMeta::new_readonly(event, false),
            AccountMeta::new_readonly(pk(&leg.authority)?, false),
            AccountMeta::new(pk(&leg.market_vault)?, false),
            AccountMeta::new_readonly(position(&event, &a.vault), false),
            AccountMeta::new(win_claim(&event, &a.vault), false),
        ],
        data,
    })
}

/// Admin-only lifecycle instruction: `close_deposits` or `settle`.
pub fn admin_ix(name: &str, a: &VaultAddresses, admin: &Pubkey) -> Instruction {
    Instruction {
        program_id: a.program,
        accounts: vec![
            AccountMeta::new_readonly(*admin, true),
            AccountMeta::new(a.basket, false),
        ],
        data: disc(name),
    }
}

/// Find a Panta market's vault authority, USDC vault and the protocol fee
/// account by reading one of its past `primary_order_usdc` transactions
/// (accounts #3, #4, #8). The market vault must be the authority's USDC ATA.
pub async fn discover_market(
    solana: &SolanaCtx,
    event: &Pubkey,
    usdc: &Pubkey,
) -> Result<(Pubkey, Pubkey, Pubkey), TxError> {
    let sigs = solana
        .rpc_json(
            "getSignaturesForAddress",
            serde_json::json!([event.to_string(), {"limit": 25}]),
        )
        .await?;
    for s in sigs.as_array().into_iter().flatten() {
        if !s["err"].is_null() {
            continue;
        }
        let tx = solana
            .rpc_json(
                "getTransaction",
                serde_json::json!([s["signature"], {"encoding": "json", "maxSupportedTransactionVersion": 0}]),
            )
            .await?;
        // Full key list: static keys, then ALT-loaded writable + readonly.
        let mut keys: Vec<String> = Vec::new();
        for list in [
            &tx["transaction"]["message"]["accountKeys"],
            &tx["meta"]["loadedAddresses"]["writable"],
            &tx["meta"]["loadedAddresses"]["readonly"],
        ] {
            keys.extend(
                list.as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|k| k.as_str().map(String::from)),
            );
        }
        for ix in tx["transaction"]["message"]["instructions"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let program = ix["programIdIndex"]
                .as_u64()
                .and_then(|i| keys.get(i as usize));
            if program.map(String::as_str) != Some(PANTA_PROGRAM) {
                continue;
            }
            let data = bs58::decode(ix["data"].as_str().unwrap_or_default())
                .into_vec()
                .unwrap_or_default();
            let accts: Vec<&String> = ix["accounts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|i| i.as_u64().and_then(|i| keys.get(i as usize)))
                .collect();
            if data.starts_with(&PRIMARY_ORDER_USDC)
                && accts.len() == 12
                && accts[1] == &event.to_string()
            {
                let authority = pk(accts[3])?;
                let market_vault = pk(accts[4])?;
                let fee = pk(accts[8])?;
                if market_vault != ata(&authority, usdc) {
                    return Err(TxError::Invalid(
                        "market vault is not the authority's ATA".into(),
                    ));
                }
                return Ok((authority, market_vault, fee));
            }
        }
    }
    Err(TxError::Invalid(format!(
        "no Panta buy found for market {event}; it needs at least one trade before tokenizing"
    )))
}

/// On-chain NAV of a vault basket: (nav in USDC base units, token supply).
pub async fn read_nav(
    solana: &SolanaCtx,
    a: &VaultAddresses,
    legs: &[VaultLeg],
) -> Result<(u64, u64), TxError> {
    let mut keys = vec![a.vault_usdc, a.mint];
    for leg in legs {
        let event = pk(&leg.event)?;
        keys.push(event);
        keys.push(position(&event, &a.vault));
    }
    let data = solana.accounts_data(&keys).await?;
    let u64_at = |d: &Option<Vec<u8>>, off: usize| -> u64 {
        d.as_ref()
            .and_then(|d| d.get(off..off + 8))
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .unwrap_or(0)
    };
    let cash = u64_at(&data[0], 64); // token account amount
    let supply = u64_at(&data[1], 36); // mint supply
    let mut nav = cash as u128;
    for (i, leg) in legs.iter().enumerate() {
        let yes = u64_at(&data[2 + 2 * i], EVENT_YES_PRICE) as u128;
        let price = if leg.side == 0 {
            yes
        } else {
            PRICE_ONE.saturating_sub(yes)
        };
        let shares = u64_at(
            &data[3 + 2 * i],
            if leg.side == 0 {
                POSITION_YES_SHARES
            } else {
                POSITION_NO_SHARES
            },
        ) as u128;
        nav += shares * price / PRICE_ONE;
    }
    Ok((nav as u64, supply))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminators_match_anchor() {
        // Panta's own instruction uses the same scheme.
        assert_eq!(disc("primary_order_usdc"), PRIMARY_ORDER_USDC.to_vec());
    }

    #[test]
    fn panta_config_pda_matches_mainnet() {
        assert_eq!(
            panta_config().to_string(),
            "8mJjfx7SuWwS4yfVZXtqC2cvzxr3TQHsCgKyjpuDWKAP"
        );
    }

    #[test]
    fn deposit_layout() {
        let program = Pubkey::new_unique();
        let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let a = addresses(&program, 7, &usdc);
        let leg = |n: u8| VaultLeg {
            market_id: "m".into(),
            event: Pubkey::new_from_array([n; 32]).to_string(),
            authority: Pubkey::new_from_array([n + 10; 32]).to_string(),
            market_vault: Pubkey::new_from_array([n + 20; 32]).to_string(),
            side: 0,
            weight_bps: 5_000,
        };
        let user = Pubkey::new_unique();
        let ix = deposit_ix(&a, &user, &[leg(1), leg(2)], &user, &user, &usdc, 5, 1).unwrap();
        assert_eq!(ix.accounts.len(), 15 + 2 * 4);
        assert_eq!(&ix.data[..8], &disc("deposit")[..]);
        assert!(ix.accounts[0].is_signer);
    }
}
