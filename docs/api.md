# Tenor API — frontend contract

Base URL: `VITE_API_BASE_URL` (local: `http://localhost:8080`). JSON everywhere; field names are
`snake_case`. **TypeScript types are generated from the Rust structs** — run `./scripts/typegen.sh`
(writes `frontend/src/api/generated/`); never hand-write them.

## Conventions

- **Money** (`*_usdc`): integer **string** of USDC base units (6 decimals). `"1500000"` = 1.50 USDC.
- **Prices**: decimal strings `0..1`. Show as cents + implied %: `"0.62"` → `62¢ · 62%`.
- **Shares**: integer strings, 6 decimals (like USDC).
- **Times**: RFC 3339 UTC.
- **Errors**: non-2xx → `{ "error": { "code", "message", "details"? } }`. Branch on `code`.
- **Auth**: `Authorization: Bearer <token>` on every user-scoped route (marked 🔒).
- Show **"Powered by Panta"** (link https://panta.market) on market pages, basket pages, buy
  panels and the footer. Show `stale: true` data as "price as of …".

## Sign in (Sign-In With Solana)

1. `POST /v1/auth/nonce` `{ wallet }` → `{ nonce, message, expires_at }`
2. Wallet `signMessage(utf8(message))`, base58-encode the 64-byte signature.
3. `POST /v1/auth/verify` `{ wallet, signature }` → `{ token, expires_at }` (24 h).

## Signing flow (orders, baskets, claims)

Every trade is **build → sign → submit**:

1. Build returns `transactions: [base64]` (unsigned v0 txs) + `summary` (show `summary_lines`,
   fees and `notices` before the wallet prompt — brief §12).
2. Deserialize with `VersionedTransaction.deserialize(base64)`, call wallet
   **`signTransaction`** (NOT `signAndSendTransaction` — we broadcast), re-serialize, base64.
3. Submit `{ signed_txs: [base64] }`. The server only accepts the **exact** transaction it built.
4. Submit waits up to ~20 s. If the response status is still `submitted`, **poll** the status
   route every 2–3 s until `filled` / `confirmed` / `failed`.

Error codes to handle: `QUOTE_STALE` / `QUOTE_EXPIRED` (re-quote), `SLIPPAGE_EXCEEDED`,
`SIMULATION_FAILED` (usually not enough USDC/SOL), `TX_VALIDATION_FAILED`, `DUPLICATE_TX`,
`ALREADY_SUBMITTED`, `TX_EXPIRED` (status `failed` with this `error_code`), `RATE_LIMITED`,
`PANTA_UNAVAILABLE`, `LEG_NOT_TRADABLE`, `AMOUNT_OUT_OF_RANGE`, `INVALID_SLIPPAGE`.

## Markets (public)

| Route | Returns |
|---|---|
| `GET /v1/categories` | `Category[]` |
| `GET /v1/markets?category=&q=&sort=volume\|newest\|closing_soon&cursor=&limit=` | `Page<MarketSummary>` (`next_cursor` → pass back as `cursor`) |
| `GET /v1/markets/{id}` | `MarketDetail` (incl. `in_baskets`, `rules_text`, `creator`) |
| `GET /v1/markets/{id}/history?range=1d\|7d\|all` | `MarketHistory` — our own snapshots; empty → "history since we started tracking" |
| `GET /v1/markets/{id}/trades` | `MarketTrade[]` (cached 30 s; `amount_usdc` and `price` may be null) |

## Single-market buy 🔒 (no Tenor fee)

| Route | Body | Returns |
|---|---|---|
| `POST /v1/orders/quote` | `{ market_id, side: "yes"\|"no", amount_usdc }` (0.01–1000 USDC) | `OrderQuote` — `quote_id` is our order id |
| `POST /v1/orders/{quote_id}/build` | — | `BuildResponse { order_id, transactions, summary, expires_at }` |
| `POST /v1/orders/{order_id}/submit` | `{ signed_txs }` (exactly 1) | `OrderView` |
| `GET /v1/orders/{order_id}` | — | `OrderView` (`status`: quoted, built, submitted, confirmed, filled, failed) |

## Baskets (basket-as-UX) — browse public, trade 🔒

| Route | Body | Returns |
|---|---|---|
| `GET /v1/baskets` | — | `BasketTemplateSummary[]` |
| `GET /v1/baskets/{slug}` | — | `BasketTemplateDetail` (legs with live prices, `index_price`) |
| `POST /v1/baskets/{template_id}/quote` | `{ amount_usdc, slippage_bps? (1–1000, default 150) }` | `BasketQuote` (per-leg breakdown, our fee, Panta fees) |
| `POST /v1/baskets/quotes/{quote_id}/build` | — | `BasketBuildResponse { purchase_id, transactions, summary }` — a quote builds once |
| `POST /v1/baskets/purchases/{purchase_id}/submit` | `{ signed_txs }` (exactly 1) | `BasketPurchaseView` |
| `GET /v1/baskets/purchases/{purchase_id}` | — | `BasketPurchaseView` (per-leg `status`) |

A basket is ONE transaction: all legs fill or none do.

## Portfolio & claims 🔒

| Route | Body | Returns |
|---|---|---|
| `GET /v1/portfolio` | — | `Portfolio { single_positions, baskets, totals }`. Unpriced positions are valued at cost. `modified_outside` → "basket modified outside Tenor". |
| `GET /v1/portfolio/activity` | — | `ActivityItem[]` (newest first, 50) |
| `POST /v1/claims/build` | `{ market_ids: [] }` (empty = claim all) | `ClaimsBuildResponse { claim_id, market_ids, remaining_market_ids, transactions, summary }` |
| `POST /v1/claims/submit` | `{ claim_id, signed_txs }` | `ClaimsSubmitResponse { claim_id, signatures, status }` |

"Claim all": if `remaining_market_ids` is non-empty after a submit, call build again with them.

## Resolution Center (public)

| Route | Returns |
|---|---|
| `GET /v1/resolution/markets?status=resolving_soon\|resolved\|delayed` | `ResolutionMarketRow[]` |
| `GET /v1/resolution/markets/{id}` | `ResolutionMarketDetail` (timeline, creator scorecard) |
| `GET /v1/resolution/creators/{creator}` | `CreatorDetail` |

Every field with `source: "app_analysis"` must be labelled **"Tenor analysis"** (the wire value
stays `app_analysis`); `source: "panta"` is Panta's official data. We cannot change a resolution.

## Admin 🔒 (wallet must be in `ADMIN_WALLETS`, else 403)

`GET /v1/admin/panta-markets?q=` · `POST /v1/admin/baskets` · `PATCH /v1/admin/baskets/{id}` ·
`POST /v1/admin/baskets/{id}/publish` · `POST /v1/admin/baskets/{id}/close`

## Health

`GET /healthz` · `GET /readyz` → `{ db, rpc, panta }`

## Tokenized baskets (vault) — on-chain basket tokens (ADR 0002)

A tokenized basket is a published basket backed by the `basket_vault` program: deposits buy every
leg into a program vault and mint an SPL **basket token** priced at NAV. Tokens redeem for USDC
after every market resolves (Panta has no sell, so redeem opens only once the vault is settled).
Disabled (`VAULT_DISABLED`) in mock mode or when `VAULT_PROGRAM_ID` is unset.

| Route | Body | Returns |
|---|---|---|
| `GET /v1/vault/baskets/{slug}` | — | `VaultBasketView` (`status`: pending, live, closed, settled; `mint`; legs; `nav_usdc`, `supply`, `nav_per_token` read on chain — label "Tenor analysis") |
| 🔒 `POST /v1/vault/baskets/{template_id}/deposit` | `{ amount_usdc (1–1000 USDC), slippage_bps? }` | `VaultBuildResponse { tx_id, transactions, summary, expected_tokens, min_tokens_out }` |
| 🔒 `POST /v1/vault/baskets/{template_id}/redeem` | `{ tokens }` | `VaultBuildResponse` (only when `status == "settled"`) |
| 🔒 `POST /v1/vault/txs/{tx_id}/submit` | `{ signed_txs }` | `VaultTxView` (`status`: built, submitted, confirmed, failed) |
| 🔒 `GET /v1/vault/txs/{tx_id}` | — | `VaultTxView` — poll while `submitted` |

Show `expected_tokens` before signing; the transaction itself reverts if fewer than
`min_tokens_out` would be minted (`Slippage`). Errors: `VAULT_NOT_LIVE`, `VAULT_NOT_SETTLED`,
`VAULT_DISABLED`, plus the signing-flow codes above.

Admin (same `build → sign → submit` via `/v1/vault/txs/{tx_id}/submit`):
`POST /v1/admin/baskets/{id}/tokenize` (published template, ≤ 3 legs; admin pays vault rent) ·
`POST /v1/admin/baskets/{id}/vault-close` · `POST /v1/admin/baskets/{id}/vault-settle`.
