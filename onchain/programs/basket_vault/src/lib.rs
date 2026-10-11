//! Tokenized baskets (Phase 3): one vault per basket holds the Panta
//! positions of every leg and mints an SPL "basket token" to depositors.
//!
//! Pricing is NAV-based (ADR 0002): on each deposit the vault buys every
//! leg, then values ALL holdings — old and new — at the post-trade Panta
//! prices. The depositor receives `supply × value_added / value_before`
//! tokens (the first deposit mints 1 token per USDC of value), so earlier
//! holders are never diluted by later buyers' price impact, and
//! `min_tokens_out` is the on-chain slippage guard Panta itself lacks.
//!
//! Panta has no sell: tokens redeem for USDC after the basket settles (the
//! vault claims every winning leg via CPI, then `redeem` pays pro rata).
//!
//! PDAs (per basket):
//! - `basket` = ["basket", id]            program-owned state
//! - `vault`  = ["vault", basket]         system-owned, no data: Panta's
//!   "buyer" (it signs Panta CPIs and pays Panta's position rent in SOL)
//! - `mint`   = ["mint", basket]          basket token, authority = basket

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

pub mod errors;
pub mod panta;

use errors::VaultError;

declare_id!("AGrL8gCofvFTvsLDXmwThAWd1Wdz9ULPYwHWCzVDnF7u");

/// Mainnet USDC.
pub const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const MAX_LEGS: usize = 4;
/// Accounts each leg contributes to `remaining_accounts`:
/// event, market authority, market vault, vault's UserPosition (+ WinClaim on claim).
pub const ACCOUNTS_PER_LEG: usize = 4;
/// Smallest deposit: 1 USDC.
pub const MIN_DEPOSIT: u64 = 1_000_000;
/// Our fee cap (5%).
pub const MAX_FEE_BPS: u16 = 500;
/// Rent the vault PDA must hold per leg: Panta's UserPosition (202 bytes)
/// on the first buy and WinClaim (121 bytes) on claim.
const POSITION_SPACE: usize = 202;
const WIN_CLAIM_SPACE: usize = 121;

/// After the last leg resolves, anyone may settle once this much time has
/// passed — the window for the (permissionless) claim crank to collect
/// every winning leg.
pub const SETTLE_GRACE_SECS: i64 = 72 * 3600;

pub const STATUS_OPEN: u8 = 0;
pub const STATUS_CLOSED: u8 = 1;
pub const STATUS_SETTLED: u8 = 2;

#[program]
pub mod basket_vault {
    use super::*;

    /// Admin creates a basket: legs, fee, token mint, vault USDC account,
    /// and funds the vault PDA with the SOL Panta will need for rent.
    pub fn create_basket(
        ctx: Context<CreateBasket>,
        id: u64,
        legs: Vec<Leg>,
        fee_bps: u16,
    ) -> Result<()> {
        validate_legs(&legs)?;
        require!(fee_bps <= MAX_FEE_BPS, VaultError::Fee);

        let basket = &mut ctx.accounts.basket;
        basket.admin = ctx.accounts.admin.key();
        basket.id = id;
        basket.mint = ctx.accounts.mint.key();
        basket.fee_usdc = ctx.accounts.fee_usdc.key();
        basket.fee_bps = fee_bps;
        basket.status = STATUS_OPEN;
        basket.bump = ctx.bumps.basket;
        basket.vault_bump = ctx.bumps.vault;
        basket.legs = legs;

        // Vault PDA rent budget: rent-exempt base + position + win-claim per leg.
        let rent = Rent::get()?;
        let needed = rent.minimum_balance(0)
            + basket.legs.len() as u64
                * (rent.minimum_balance(POSITION_SPACE) + rent.minimum_balance(WIN_CLAIM_SPACE));
        let have = ctx.accounts.vault.lamports();
        if have < needed {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.key(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.admin.to_account_info(),
                        to: ctx.accounts.vault.to_account_info(),
                    },
                ),
                needed - have,
            )?;
        }
        Ok(())
    }

    /// Deposit USDC: our fee off the top, the rest buys every leg by
    /// weight; mint basket tokens at NAV (see module docs).
    ///
    /// `remaining_accounts`: for each leg, in basket order:
    /// [event (w), market authority, market vault (w), vault's UserPosition (w)].
    pub fn deposit<'info>(
        ctx: Context<'info, Deposit<'info>>,
        amount: u64,
        min_tokens_out: u64,
    ) -> Result<()> {
        let basket = &ctx.accounts.basket;
        require!(basket.status == STATUS_OPEN, VaultError::NotOpen);
        require!(amount >= MIN_DEPOSIT, VaultError::AmountTooSmall);
        let legs = leg_accounts(
            &basket.legs,
            ctx.remaining_accounts,
            &ctx.accounts.vault.key(),
        )?;

        // 1. Holdings before (shares per leg + idle vault cash).
        let vault_key = ctx.accounts.vault.key();
        let mut shares_before = Vec::with_capacity(legs.len());
        for (leg, accs) in basket.legs.iter().zip(&legs) {
            shares_before.push(panta::position_shares(accs.position, &vault_key, leg.side)?);
        }
        let cash_before = ctx.accounts.vault_usdc.amount;

        // 2. Move the money: our fee, then the net amount into the vault.
        let fee = (amount as u128 * basket.fee_bps as u128 / 10_000) as u64;
        let net = amount - fee;
        if fee > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.user_usdc.to_account_info(),
                        to: ctx.accounts.fee_usdc.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                fee,
            )?;
        }
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Transfer {
                    from: ctx.accounts.user_usdc.to_account_info(),
                    to: ctx.accounts.vault_usdc.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            net,
        )?;

        // 3. Buy every leg. Panta's fee is charged on top of the order, so
        //    each leg's order is sized to leave room for it.
        let basket_key = basket.key();
        let vault_seeds: &[&[u8]] = &[b"vault", basket_key.as_ref(), &[basket.vault_bump]];
        let shared = shared_accounts(&ctx)?;
        for (leg, accs) in basket.legs.iter().zip(&legs) {
            let allocation = (net as u128 * leg.weight_bps as u128 / 10_000) as u64;
            let order = allocation * 10_000 / (10_000 + panta::PRIMARY_FEE_BPS);
            require!(order > 0, VaultError::AmountTooSmall);
            panta::buy(&shared, accs, leg.side, order, &[vault_seeds])?;
        }

        // 4. Holdings after, all valued at post-trade prices.
        ctx.accounts.vault_usdc.reload()?;
        let cash_after = ctx.accounts.vault_usdc.amount;
        let mut holdings = Vec::with_capacity(legs.len());
        for ((leg, accs), before) in basket.legs.iter().zip(&legs).zip(&shares_before) {
            let price = panta::side_price(accs.event, leg.side)?;
            let after = panta::position_shares(accs.position, &vault_key, leg.side)?;
            holdings.push((*before, after, price));
        }
        let minted =
            tokens_for_deposit(ctx.accounts.mint.supply, cash_before, cash_after, &holdings)?;
        require!(minted > 0, VaultError::Zero);
        require!(minted >= min_tokens_out, VaultError::Slippage);

        // 5. Mint to the depositor (basket PDA is the mint authority).
        let id_bytes = basket.id.to_le_bytes();
        let basket_seeds: &[&[u8]] = &[b"basket", id_bytes.as_ref(), &[basket.bump]];
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.user_basket.to_account_info(),
                    authority: ctx.accounts.basket.to_account_info(),
                },
                &[basket_seeds],
            ),
            minted,
        )?;

        emit!(Deposited {
            basket: basket_key,
            user: ctx.accounts.user.key(),
            amount,
            fee,
            minted,
        });
        Ok(())
    }

    /// Stop new deposits (e.g. a leg left Panta's primary phase). Admin only.
    pub fn close_deposits(ctx: Context<AdminOnly>) -> Result<()> {
        require!(
            ctx.accounts.basket.status == STATUS_OPEN,
            VaultError::NotOpen
        );
        ctx.accounts.basket.status = STATUS_CLOSED;
        Ok(())
    }

    /// Claim one resolved, winning leg into the vault's USDC (anyone may
    /// crank this; the payout can only go to the vault).
    ///
    /// `remaining_accounts`: [event, market authority, market vault (w),
    /// vault's UserPosition, vault's WinClaim (w)].
    pub fn claim_leg<'info>(ctx: Context<'info, ClaimLeg<'info>>, leg_index: u8) -> Result<()> {
        let basket = &ctx.accounts.basket;
        let leg = basket
            .legs
            .get(leg_index as usize)
            .ok_or(error!(VaultError::LegAccounts))?;
        let rem = ctx.remaining_accounts;
        require!(rem.len() == ACCOUNTS_PER_LEG + 1, VaultError::LegAccounts);
        let vault_key = ctx.accounts.vault.key();
        require_keys_eq!(rem[0].key(), leg.event, VaultError::LegAccounts);
        require_keys_eq!(
            rem[3].key(),
            panta::position_address(&leg.event, &vault_key),
            VaultError::LegAccounts
        );
        require_keys_eq!(
            rem[4].key(),
            panta::win_claim_address(&leg.event, &vault_key),
            VaultError::LegAccounts
        );
        let accs = panta::LegAccounts {
            event: &rem[0],
            market_authority: &rem[1],
            market_vault: &rem[2],
            position: &rem[3],
        };
        let basket_key = basket.key();
        let vault_seeds: &[&[u8]] = &[b"vault", basket_key.as_ref(), &[basket.vault_bump]];
        let shared = panta::Shared {
            buyer: &ctx.accounts.vault.to_account_info(),
            buyer_usdc: &ctx.accounts.vault_usdc.to_account_info(),
            config: &ctx.accounts.panta_config.to_account_info(),
            fee_account: &ctx.accounts.panta_config.to_account_info(), // unused by claim
            usdc_mint: &ctx.accounts.usdc_mint.to_account_info(),
            token_program: &ctx.accounts.token_program.to_account_info(),
            ata_program: &ctx.accounts.associated_token_program.to_account_info(),
            system_program: &ctx.accounts.system_program.to_account_info(),
            panta_program: &ctx.accounts.panta_program.to_account_info(),
        };
        panta::claim(&shared, &accs, &rem[4], &[vault_seeds])
    }

    /// Admin settles early (after claiming winners), but only once every
    /// leg's market has resolved. `remaining_accounts`: each leg's Panta Event.
    pub fn settle<'info>(ctx: Context<'info, AdminOnly<'info>>) -> Result<()> {
        latest_resolution(&ctx.accounts.basket.legs, ctx.remaining_accounts)?;
        mark_settled(&mut ctx.accounts.basket)
    }

    /// Anyone settles once every leg resolved AND the claim grace period
    /// passed since the last resolution — no admin needed (ADR 0002).
    /// `remaining_accounts`: each leg's Panta Event.
    pub fn settle_after_grace<'info>(ctx: Context<'info, Permissionless<'info>>) -> Result<()> {
        let last = latest_resolution(&ctx.accounts.basket.legs, ctx.remaining_accounts)?;
        let now = Clock::get()?.unix_timestamp;
        require!(now >= last + SETTLE_GRACE_SECS, VaultError::GracePeriod);
        mark_settled(&mut ctx.accounts.basket)
    }

    /// Burn basket tokens for a pro-rata share of the vault's USDC.
    pub fn redeem(ctx: Context<Redeem>, tokens: u64) -> Result<()> {
        let basket = &ctx.accounts.basket;
        require!(basket.status == STATUS_SETTLED, VaultError::NotSettled);
        let payout = redemption_payout(
            tokens,
            ctx.accounts.mint.supply,
            ctx.accounts.vault_usdc.amount,
        )?;

        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.user_basket.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            tokens,
        )?;
        if payout > 0 {
            let basket_key = basket.key();
            let vault_seeds: &[&[u8]] = &[b"vault", basket_key.as_ref(), &[basket.vault_bump]];
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.vault_usdc.to_account_info(),
                        to: ctx.accounts.user_usdc.to_account_info(),
                        authority: ctx.accounts.vault.to_account_info(),
                    },
                    &[vault_seeds],
                ),
                payout,
            )?;
        }
        emit!(Redeemed {
            basket: basket.key(),
            user: ctx.accounts.user.key(),
            tokens,
            payout,
        });
        Ok(())
    }
}

// --- pure math (unit-tested below) ------------------------------------------

/// Basket tokens for a deposit. `holdings` = (shares_before, shares_after,
/// post-trade price 1e9-scaled) per leg. Everything is valued at the SAME
/// post-trade prices, so the depositor pays for their own price impact.
pub fn tokens_for_deposit(
    supply: u64,
    cash_before: u64,
    cash_after: u64,
    holdings: &[(u64, u64, u128)],
) -> Result<u64> {
    let value = |shares: u64, price: u128| shares as u128 * price / panta::PRICE_ONE;
    let mut before: u128 = cash_before as u128;
    let mut after: u128 = cash_after as u128;
    for (b, a, p) in holdings {
        before += value(*b, *p);
        after += value(*a, *p);
    }
    let added = after
        .checked_sub(before)
        .ok_or(error!(VaultError::Overflow))?;
    let minted = if supply == 0 {
        added // first deposit: 1 token (6 decimals) per USDC of value
    } else {
        require!(before > 0, VaultError::Zero);
        added
            .checked_mul(supply as u128)
            .ok_or(error!(VaultError::Overflow))?
            / before
    };
    u64::try_from(minted).map_err(|_| error!(VaultError::Overflow))
}

/// USDC paid for burning `tokens` out of `supply`, from `vault_cash`.
pub fn redemption_payout(tokens: u64, supply: u64, vault_cash: u64) -> Result<u64> {
    require!(
        tokens > 0 && supply > 0 && tokens <= supply,
        VaultError::Zero
    );
    Ok((vault_cash as u128 * tokens as u128 / supply as u128) as u64)
}

pub fn validate_legs(legs: &[Leg]) -> Result<()> {
    require!((2..=MAX_LEGS).contains(&legs.len()), VaultError::LegCount);
    let mut total: u32 = 0;
    for (i, leg) in legs.iter().enumerate() {
        require!(leg.side <= 1, VaultError::Side);
        require!(leg.weight_bps > 0, VaultError::Weights);
        total += leg.weight_bps as u32;
        require!(
            legs[..i].iter().all(|other| other.event != leg.event),
            VaultError::DuplicateLeg
        );
    }
    require!(total == 10_000, VaultError::Weights);
    Ok(())
}

/// Latest `resolved_at` across the legs; errors if any leg is unresolved.
fn latest_resolution(legs: &[Leg], events: &[AccountInfo]) -> Result<i64> {
    require!(events.len() == legs.len(), VaultError::LegAccounts);
    let mut last = 0;
    for (leg, event) in legs.iter().zip(events) {
        require_keys_eq!(event.key(), leg.event, VaultError::LegAccounts);
        let at = panta::resolved_at(event)?.ok_or(error!(VaultError::NotResolved))?;
        last = last.max(at);
    }
    Ok(last)
}

fn mark_settled(basket: &mut Basket) -> Result<()> {
    require!(basket.status != STATUS_SETTLED, VaultError::NotOpen);
    basket.status = STATUS_SETTLED;
    Ok(())
}

/// Split and check `remaining_accounts` against the basket's legs.
fn leg_accounts<'a, 'info>(
    legs: &[Leg],
    rem: &'a [AccountInfo<'info>],
    vault: &Pubkey,
) -> Result<Vec<panta::LegAccounts<'a, 'info>>> {
    require!(
        rem.len() == legs.len() * ACCOUNTS_PER_LEG,
        VaultError::LegAccounts
    );
    legs.iter()
        .zip(rem.chunks(ACCOUNTS_PER_LEG))
        .map(|(leg, c)| {
            require_keys_eq!(c[0].key(), leg.event, VaultError::LegAccounts);
            require_keys_eq!(
                c[3].key(),
                panta::position_address(&leg.event, vault),
                VaultError::LegAccounts
            );
            Ok(panta::LegAccounts {
                event: &c[0],
                market_authority: &c[1],
                market_vault: &c[2],
                position: &c[3],
            })
        })
        .collect()
}

fn shared_accounts<'a, 'info>(
    ctx: &'a Context<'info, Deposit<'info>>,
) -> Result<panta::Shared<'a, 'info>> {
    Ok(panta::Shared {
        buyer: ctx.accounts.vault.as_ref(),
        buyer_usdc: (*ctx.accounts.vault_usdc).as_ref(),
        config: ctx.accounts.panta_config.as_ref(),
        fee_account: ctx.accounts.panta_fee_usdc.as_ref(),
        usdc_mint: (*ctx.accounts.usdc_mint).as_ref(),
        token_program: ctx.accounts.token_program.as_ref(),
        ata_program: ctx.accounts.associated_token_program.as_ref(),
        system_program: ctx.accounts.system_program.as_ref(),
        panta_program: ctx.accounts.panta_program.as_ref(),
    })
}

// --- state ------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace, PartialEq, Eq, Debug)]
pub struct Leg {
    /// Panta `Event` account of the market.
    pub event: Pubkey,
    /// 0 = YES, 1 = NO.
    pub side: u8,
    pub weight_bps: u16,
}

#[account]
#[derive(InitSpace)]
pub struct Basket {
    pub admin: Pubkey,
    pub id: u64,
    pub mint: Pubkey,
    /// Our fee destination (fee wallet's USDC ATA).
    pub fee_usdc: Pubkey,
    pub fee_bps: u16,
    pub status: u8,
    pub bump: u8,
    pub vault_bump: u8,
    #[max_len(MAX_LEGS)]
    pub legs: Vec<Leg>,
}

#[event]
pub struct Deposited {
    pub basket: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub fee: u64,
    pub minted: u64,
}

#[event]
pub struct Redeemed {
    pub basket: Pubkey,
    pub user: Pubkey,
    pub tokens: u64,
    pub payout: u64,
}

// --- accounts ---------------------------------------------------------------

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CreateBasket<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        init,
        payer = admin,
        space = 8 + Basket::INIT_SPACE,
        seeds = [b"basket", id.to_le_bytes().as_ref()],
        bump,
    )]
    pub basket: Box<Account<'info, Basket>>,
    /// CHECK: system-owned PDA with no data (Panta's buyer); only receives SOL here.
    #[account(mut, seeds = [b"vault", basket.key().as_ref()], bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(
        init,
        payer = admin,
        seeds = [b"mint", basket.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = basket,
    )]
    pub mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = usdc_mint,
        associated_token::authority = vault,
    )]
    pub vault_usdc: Box<Account<'info, TokenAccount>>,
    #[account(token::mint = usdc_mint)]
    pub fee_usdc: Box<Account<'info, TokenAccount>>,
    #[account(address = USDC_MINT)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(seeds = [b"basket", basket.id.to_le_bytes().as_ref()], bump = basket.bump)]
    pub basket: Box<Account<'info, Basket>>,
    /// CHECK: vault PDA (seeds checked); signs Panta CPIs.
    #[account(mut, seeds = [b"vault", basket.key().as_ref()], bump = basket.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut, address = basket.mint)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = vault,
    )]
    pub vault_usdc: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = usdc_mint, token::authority = user)]
    pub user_usdc: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint, token::authority = user)]
    pub user_basket: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = basket.fee_usdc)]
    pub fee_usdc: Box<Account<'info, TokenAccount>>,
    #[account(address = USDC_MINT)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    /// CHECK: Panta MarketConfig; validated by Panta.
    #[account(owner = panta::PROGRAM_ID)]
    pub panta_config: UncheckedAccount<'info>,
    /// CHECK: Panta protocol fee USDC account; validated by Panta.
    #[account(mut)]
    pub panta_fee_usdc: UncheckedAccount<'info>,
    /// CHECK: the Panta program itself.
    #[account(address = panta::PROGRAM_ID, executable)]
    pub panta_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimLeg<'info> {
    #[account(seeds = [b"basket", basket.id.to_le_bytes().as_ref()], bump = basket.bump)]
    pub basket: Box<Account<'info, Basket>>,
    /// CHECK: vault PDA (seeds checked); signs the Panta claim.
    #[account(mut, seeds = [b"vault", basket.key().as_ref()], bump = basket.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = vault,
    )]
    pub vault_usdc: Box<Account<'info, TokenAccount>>,
    #[account(address = USDC_MINT)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    /// CHECK: Panta MarketConfig; validated by Panta.
    #[account(owner = panta::PROGRAM_ID)]
    pub panta_config: UncheckedAccount<'info>,
    /// CHECK: the Panta program itself.
    #[account(address = panta::PROGRAM_ID, executable)]
    pub panta_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AdminOnly<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        has_one = admin,
        seeds = [b"basket", basket.id.to_le_bytes().as_ref()],
        bump = basket.bump,
    )]
    pub basket: Box<Account<'info, Basket>>,
}

#[derive(Accounts)]
pub struct Permissionless<'info> {
    #[account(
        mut,
        seeds = [b"basket", basket.id.to_le_bytes().as_ref()],
        bump = basket.bump,
    )]
    pub basket: Box<Account<'info, Basket>>,
}

#[derive(Accounts)]
pub struct Redeem<'info> {
    pub user: Signer<'info>,
    #[account(seeds = [b"basket", basket.id.to_le_bytes().as_ref()], bump = basket.bump)]
    pub basket: Box<Account<'info, Basket>>,
    /// CHECK: vault PDA (seeds checked); authority of vault_usdc.
    #[account(seeds = [b"vault", basket.key().as_ref()], bump = basket.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut, address = basket.mint)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = vault,
    )]
    pub vault_usdc: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = usdc_mint, token::authority = user)]
    pub user_usdc: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint, token::authority = user)]
    pub user_basket: Box<Account<'info, TokenAccount>>,
    #[account(address = USDC_MINT)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    pub token_program: Program<'info, Token>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: u128 = panta::PRICE_ONE;

    #[test]
    fn first_deposit_mints_value() {
        // 2 legs, cash 0 -> 0.1 USDC dust; legs bought 2 shares @0.5 and 4 shares @0.25.
        let minted = tokens_for_deposit(
            0,
            0,
            100_000,
            &[(0, 2_000_000, P / 2), (0, 4_000_000, P / 4)],
        )
        .unwrap();
        assert_eq!(minted, 2_100_000); // 1 + 1 + 0.1 USDC of value
    }

    #[test]
    fn later_deposit_pays_its_own_price_impact() {
        // Existing: 1_000_000 tokens backed by 2 shares. Depositor's buy moved
        // the price 0.50 -> 0.60 and got 1.5 shares. Old holdings are revalued
        // at 0.60 too, so the depositor gets tokens only for their 1.5 shares.
        let minted =
            tokens_for_deposit(1_000_000, 0, 0, &[(2_000_000, 3_500_000, P * 6 / 10)]).unwrap();
        // value_before = 1.2, added = 0.9 -> 1_000_000 * 0.9 / 1.2 = 750_000
        assert_eq!(minted, 750_000);
    }

    #[test]
    fn redemption_is_pro_rata() {
        assert_eq!(redemption_payout(250, 1_000, 4_000_000).unwrap(), 1_000_000);
        assert!(redemption_payout(0, 1_000, 4_000_000).is_err());
        assert!(redemption_payout(2_000, 1_000, 4_000_000).is_err());
    }

    #[test]
    fn leg_rules() {
        let leg = |b: u8, side: u8, w: u16| Leg {
            event: Pubkey::new_from_array([b; 32]),
            side,
            weight_bps: w,
        };
        assert!(validate_legs(&[leg(1, 0, 5_000), leg(2, 1, 5_000)]).is_ok());
        assert!(validate_legs(&[leg(1, 0, 10_000)]).is_err()); // one leg
        assert!(validate_legs(&[leg(1, 0, 4_000), leg(2, 1, 5_000)]).is_err()); // sum
        assert!(validate_legs(&[leg(1, 0, 5_000), leg(1, 1, 5_000)]).is_err()); // dup
        assert!(validate_legs(&[leg(1, 2, 5_000), leg(2, 0, 5_000)]).is_err()); // side
    }
}
