# ADR 0002 — Tokenized baskets (vault program, NAV pricing)

Status: accepted (11 October 2026). Supersedes brief D4/D5 for Phase 3 ("no basket token, no smart contracts").

## Context

Basket-as-UX (D4) leaves each leg as a plain Panta position in the user's wallet. We want a
single transferable basket token. Panta exposes no sell, so a token is also the only way to
offer an early exit (secondary trading of the token).

Spike (docs/panta-notes.md): Panta's `primary_order_usdc` accepts a **PDA buyer** signing via
`invoke_signed`, paying from the PDA's off-curve USDC ATA. Positions are per-owner
`UserPosition` accounts (not SPL tokens), so the vault must hold them itself.

## Decision

On-chain program `basket_vault` (Anchor, `onchain/programs/basket_vault`):

- One `Basket` account per basket (legs, fee, status), one system-owned **vault PDA** that is
  Panta's buyer, one SPL **basket token** mint (6 decimals) per basket.
- **Deposit**: our fee off the top → remaining USDC buys every leg by weight via CPI (order
  sized for Panta's 2% fee on top) → all holdings valued at the **post-trade** Panta prices →
  depositor receives `supply × value_added / value_before` tokens (first deposit: 1 token per
  USDC of value). `min_tokens_out` is the on-chain slippage guard (Panta has none on chain).
- **Claim**: permissionless crank, `claim_win_usdc` via CPI; winnings go to the vault only.
- **Settle → redeem**: once all legs resolved and winners claimed, admin settles; holders burn
  tokens for a pro-rata share of the vault's USDC.

## Consequences / known limits (v1)

- **Admin trust for `settle`**: settling before every winning leg is claimed would short early
  redeemers. Next step: read Panta's resolution state on chain and make settle permissionless.
- **Layout coupling**: we read `Event.yes_price` (offset 152) and `UserPosition` shares
  (offsets 74/90). Panta is upgradeable; checks on discriminators/owners make a layout change
  fail closed, but it would halt deposits until we update.
- **Prices are bonding-curve marginal prices**, not fair value; NAV uses them consistently for
  every holder, which is what makes deposits non-dilutive.
- **Transaction size**: a 4-leg deposit references ~31 accounts → needs an Address Lookup
  Table per basket (backend creates it at basket creation).
- **Custody**: user funds sit in a program vault → needs an external audit before mainnet, and
  legal review (brief §14).

## Verification status (11 October 2026)

- Unit tests: NAV minting (first deposit, later deposit pays its own price impact),
  pro-rata redemption, leg rules.
- Local validator with Panta's mainnet program cloned (`onchain/e2e/run.sh`):
  `create_basket` works; `deposit` takes our fee and buys leg 1 through the vault PDA
  (Panta minted shares to the vault's `UserPosition`). The full 2-leg run needs two markets
  that are still in Panta's primary (flash acquisition) window at the same time — at test
  time only one was, so leg 2 failed with Panta's `FlashAcquisitionEnded` (6085) and the
  whole deposit reverted atomically, as designed.
- Not yet exercised: `claim_leg` (needs a resolved market where the vault holds a winning
  position), `settle`/`redeem` end to end.
