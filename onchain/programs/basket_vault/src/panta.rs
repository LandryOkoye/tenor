//! Panta program interface: ids, instruction layouts, and the two account
//! fields we read. Everything here was reverse-engineered from mainnet
//! transactions and verified on a local validator cloning the real program
//! (see docs/panta-notes.md). Panta's program is upgradeable — if they
//! change a layout, deposits fail closed (offset checks + Panta's own
//! constraints) rather than mis-pricing.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use crate::errors::VaultError;

/// Panta prediction-market program (mainnet).
pub const PROGRAM_ID: Pubkey = pubkey!("6gM5afTQBq5VZCfgpGqcsqzfWd5maLSCKWtGjbEobZMp");

/// `sha256("global:primary_order_usdc")[..8]`; args: side u8 (0 YES, 1 NO), amount u64.
const PRIMARY_ORDER_USDC: [u8; 8] = [0x2e, 0x89, 0x44, 0x74, 0x31, 0x59, 0x0d, 0xf7];
/// `sha256("global:claim_win_usdc")[..8]`; arg: claimer pubkey.
const CLAIM_WIN_USDC: [u8; 8] = [0x2b, 0xa0, 0x6a, 0x33, 0xa7, 0x4c, 0x14, 0x1f];

/// Anchor account discriminators (`sha256("account:<Name>")[..8]`).
const EVENT_DISC: [u8; 8] = [0x7d, 0xc0, 0x7d, 0x9e, 0x09, 0x73, 0x98, 0xe9];
const USER_POSITION_DISC: [u8; 8] = [0xfb, 0xf8, 0xd1, 0xf5, 0x53, 0xea, 0x11, 0x1b];

/// `Event.yes_price` (u64, 1e9 = 1 USDC). NO price = 1e9 - YES price.
const EVENT_YES_PRICE: usize = 152;
/// `UserPosition` fields.
const POSITION_USER: usize = 41;
const POSITION_YES_SHARES: usize = 74;
const POSITION_NO_SHARES: usize = 90;

/// Price scale used by Panta (1e9 = 1 USDC per share).
pub const PRICE_ONE: u128 = 1_000_000_000;
/// Panta's primary fee, charged ON TOP of the order amount (2%, read from
/// MarketConfig on chain). If Panta raises it, buys fail for lack of funds
/// and the whole deposit reverts — never a silent loss.
pub const PRIMARY_FEE_BPS: u64 = 200;

/// `UserPosition` PDA: `["position", event, owner]`.
pub fn position_address(event: &Pubkey, owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"position", event.as_ref(), owner.as_ref()], &PROGRAM_ID).0
}

/// `WinClaim` PDA: `["win_claim", event, owner]`.
pub fn win_claim_address(event: &Pubkey, owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"win_claim", event.as_ref(), owner.as_ref()], &PROGRAM_ID).0
}

fn read_u64(data: &[u8], offset: usize) -> Result<u64> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or(error!(VaultError::PantaLayout))?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

/// Current price of `side` on a Panta `Event`, 1e9-scaled.
pub fn side_price(event: &AccountInfo, side: u8) -> Result<u128> {
    require_keys_eq!(*event.owner, PROGRAM_ID, VaultError::PantaLayout);
    let data = event.try_borrow_data()?;
    require!(
        data.get(..8) == Some(&EVENT_DISC[..]),
        VaultError::PantaLayout
    );
    let yes = read_u64(&data, EVENT_YES_PRICE)? as u128;
    require!(yes <= PRICE_ONE, VaultError::PantaLayout);
    Ok(if side == 0 { yes } else { PRICE_ONE - yes })
}

/// Shares of `side` held in a `UserPosition` (0 if it does not exist yet).
/// Checks the position really belongs to `owner`.
pub fn position_shares(position: &AccountInfo, owner: &Pubkey, side: u8) -> Result<u64> {
    if position.data_is_empty() {
        return Ok(0); // created by Panta on the first buy
    }
    require_keys_eq!(*position.owner, PROGRAM_ID, VaultError::PantaLayout);
    let data = position.try_borrow_data()?;
    require!(
        data.get(..8) == Some(&USER_POSITION_DISC[..]),
        VaultError::PantaLayout
    );
    let user = data
        .get(POSITION_USER..POSITION_USER + 32)
        .ok_or(error!(VaultError::PantaLayout))?;
    require!(user == owner.as_ref(), VaultError::PantaLayout);
    read_u64(
        &data,
        if side == 0 {
            POSITION_YES_SHARES
        } else {
            POSITION_NO_SHARES
        },
    )
}

/// Accounts of one basket leg, in the order the client passes them.
pub struct LegAccounts<'a, 'info> {
    pub event: &'a AccountInfo<'info>,
    pub market_authority: &'a AccountInfo<'info>,
    pub market_vault: &'a AccountInfo<'info>,
    pub position: &'a AccountInfo<'info>,
}

/// Shared accounts every Panta CPI needs.
pub struct Shared<'a, 'info> {
    pub buyer: &'a AccountInfo<'info>, // our vault PDA
    pub buyer_usdc: &'a AccountInfo<'info>,
    pub config: &'a AccountInfo<'info>,
    pub fee_account: &'a AccountInfo<'info>,
    pub usdc_mint: &'a AccountInfo<'info>,
    pub token_program: &'a AccountInfo<'info>,
    pub ata_program: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
    pub panta_program: &'a AccountInfo<'info>,
}

/// `primary_order_usdc` signed by the vault PDA.
pub fn buy<'info>(
    shared: &Shared<'_, 'info>,
    leg: &LegAccounts<'_, 'info>,
    side: u8,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = PRIMARY_ORDER_USDC.to_vec();
    data.push(side);
    data.extend_from_slice(&amount.to_le_bytes());
    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(shared.buyer.key(), true),
            AccountMeta::new(leg.event.key(), false),
            AccountMeta::new_readonly(shared.config.key(), false),
            AccountMeta::new_readonly(leg.market_authority.key(), false),
            AccountMeta::new(leg.market_vault.key(), false),
            AccountMeta::new(leg.position.key(), false),
            AccountMeta::new_readonly(shared.usdc_mint.key(), false),
            AccountMeta::new(shared.buyer_usdc.key(), false),
            AccountMeta::new(shared.fee_account.key(), false),
            AccountMeta::new_readonly(shared.token_program.key(), false),
            AccountMeta::new_readonly(shared.ata_program.key(), false),
            AccountMeta::new_readonly(shared.system_program.key(), false),
        ],
        data,
    };
    invoke_signed(
        &ix,
        &[
            shared.buyer.clone(),
            leg.event.clone(),
            shared.config.clone(),
            leg.market_authority.clone(),
            leg.market_vault.clone(),
            leg.position.clone(),
            shared.usdc_mint.clone(),
            shared.buyer_usdc.clone(),
            shared.fee_account.clone(),
            shared.token_program.clone(),
            shared.ata_program.clone(),
            shared.system_program.clone(),
            shared.panta_program.clone(),
        ],
        signer_seeds,
    )?;
    Ok(())
}

/// `claim_win_usdc` signed by the vault PDA; winnings land in `buyer_usdc`.
pub fn claim<'info>(
    shared: &Shared<'_, 'info>,
    leg: &LegAccounts<'_, 'info>,
    win_claim: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let mut data = CLAIM_WIN_USDC.to_vec();
    data.extend_from_slice(shared.buyer.key().as_ref());
    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(shared.buyer.key(), true),
            AccountMeta::new_readonly(shared.config.key(), false),
            AccountMeta::new_readonly(leg.event.key(), false),
            AccountMeta::new_readonly(leg.market_authority.key(), false),
            AccountMeta::new(leg.market_vault.key(), false),
            AccountMeta::new_readonly(leg.position.key(), false),
            AccountMeta::new(win_claim.key(), false),
            AccountMeta::new(shared.buyer_usdc.key(), false),
            AccountMeta::new_readonly(shared.usdc_mint.key(), false),
            AccountMeta::new_readonly(shared.token_program.key(), false),
            AccountMeta::new_readonly(shared.ata_program.key(), false),
            AccountMeta::new_readonly(shared.system_program.key(), false),
        ],
        data,
    };
    invoke_signed(
        &ix,
        &[
            shared.buyer.clone(),
            shared.config.clone(),
            leg.event.clone(),
            leg.market_authority.clone(),
            leg.market_vault.clone(),
            leg.position.clone(),
            win_claim.clone(),
            shared.buyer_usdc.clone(),
            shared.usdc_mint.clone(),
            shared.token_program.clone(),
            shared.ata_program.clone(),
            shared.system_program.clone(),
            shared.panta_program.clone(),
        ],
        signer_seeds,
    )?;
    Ok(())
}
