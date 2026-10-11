# Panta API — verified notes

> Everything below was verified against https://docs.panta.market on 10 October 2026.
> Update this file whenever we learn new behaviour from real calls (brief §21).
> Source of truth for the wire types in `backend/src/panta/types.rs`.

## Basics

- Base URL: `https://live-api.panta.market/api/v1`
- **Trailing slashes are required** on every path (`/markets/`, not `/markets`).
- Auth: header `X-Api-Key: pk_test_… | pk_live_…` (or `Authorization: Bearer <jwt>`).
- **No testnet/sandbox.** Test and live keys hit the same production API; signed
  transactions move real USDC on Solana mainnet.
- Error envelope (non-2xx): `{ "code": "…", "message": "…", "field"?: …, "fields"?: … }`.

## Amount formats (differ by endpoint — handle at the boundary)

| Endpoint | Format | Example |
|---|---|---|
| `POST /primaryorderquote/` `amountUsdc` | human-readable decimal string | `"20.00"` |
| `POST /primaryorderverify/` response `amountUsdc` | **integer base units** (number) | `20000000` |
| `GET /positions/` `shares` | human-readable decimal string | `"38.40"` |
| Market list `volumeUsdc` | human-readable string | `"1200.00"` |
| Quote response `shares` / `avgPrice` / `feeUsdc` | human-readable decimal strings | `"38.42"`, `"0.520800"`, `"0.40"` |

USDC has 6 decimals. Internally we keep everything as `i64` base units and
convert with the helpers in `backend/src/panta/mod.rs` (never floats).

## Session lifetimes

| Resource | TTL |
|---|---|
| Primary quote (`quoteId`) | ~90 s |
| Primary order (`orderId`) | ~120 s |
| Build blockhash hint | ~60 s |

## Rate limit families (per account, defaults; `Retry-After` header on 429)

| Family | Routes | Max / window |
|---|---|---|
| read | catalog, categories, account reads | 120 / 60 s |
| positions | `GET /positions/` | 60 / 60 s |
| quote | create + primary quote | 30 / 60 s |
| build | create + primary + claim build | 20 / 60 s |
| register | submit, verify, trade report, auth | 40 / 60 s |

## Endpoints we use (request/response shapes in `types.rs`)

| Purpose | Method + path | Notes |
|---|---|---|
| Market catalog | `GET /markets/` | `?category=&status=&createdBy=me&cursor=&limit=` (limit max 50). List rows have `yesPrice`/`noPrice` = **null**. |
| Market detail | `GET /markets/{marketId}/` | Same row shape but with spot prices filled (when RPC available). |
| Market trade tape | `GET /markets/{marketId}/trades/` | `?limit=` (max 200). |
| Categories | `GET /categories/` | `{ "categories": ["sports", …] }`. |
| Wallet trades | `GET /wallets/{wallet}/trades/` | Same trade-row shape as market tape. |
| Positions | `GET /positions/?wallet=` | `shares`, `side`, `phase`, `claimable`, `claimed`, `outcome`. One row per (market, side). Capped ~200 rows. Indexer lags chain briefly. |
| Quote primary buy | `POST /primaryorderquote/` | `{wallet, marketId, side: "yes"\|"no", amountUsdc, userId?}` → `quoteId`, `shares`, `avgPrice`, `feeUsdc`, `expiresAt`. |
| Build primary buy | `POST /primaryorderbuild/` | `{quoteId, wallet, userId?, maxSlippageBps? (default 100, max 5000)}` → `orderId`, `instructions[]`, `recentBlockhash`, `expectedShares`, `lastValidBlockHeight`. Curve moved → `QUOTE_STALE`. |
| Submit primary buy | `POST /primaryordersubmit/` | `{orderId, signature, wallet?}` → `{status: "submitted"}`. Idempotent on (orderId, signature). |
| Verify primary buy | `POST /primaryorderverify/` | `{orderId, signature?, wallet?}` → status: `built \| submitted \| confirmed \| failed \| expired`. |
| Build win claim | `POST /claim/build/` | `{wallet, marketId}` → `outcome` ("YES"/"NO"), `winningShares`, `instructions[]`, `recentBlockhash`. Re-validates on-chain, fails closed (`NOT_CLAIMABLE`). |
| Report trade | `POST /trades/` | `{signature, wallet, marketId, quoteId?, clientOrderId?, userId?}` → `{status: "processed", kind: "buy"\|"claim", side}`. Fail-closed verification; idempotent per signature. Creator-fee claims are NOT attributable (→ `TX_MISMATCH`). |
| Trade status | `GET /trades/{signature}/` | `{status: processed \| pending_attribution \| unknown \| failed}`. |

Instruction wire shape (from build/claim responses):

```json
{ "programId": "…", "data": "<base64>", "accounts": [ { "pubkey": "…", "isSigner": true, "isWritable": true } ] }
```

## Error codes seen in docs

`UNAUTHORIZED`, `INVALID_MARKET_PARAMS`, `MARKET_NOT_FOUND`, `MARKET_NOT_IN_PRIMARY`,
`AMOUNT_TOO_SMALL`, `QUOTE_EXPIRED`, `QUOTE_STALE`, `NOT_CLAIMABLE`, `NOT_MARKET_CREATOR`,
`MARKET_NOT_GRADUATED`, `NO_CREATOR_FEES`, `TX_NOT_FOUND`, `TX_FAILED`, `TX_MISMATCH`,
`TX_FEE_MISMATCH`, `RATE_LIMITED`, `INTERNAL_ERROR`, `CREATE_NOT_PERMITTED`,
`DUPLICATE_MARKET`, `CREATE_EXPIRED`, `UPLOAD_NOT_CONFIGURED`, `EMAIL_TAKEN`, `FORBIDDEN`.

Map Panta codes through to our API responses unchanged (brief §9).

## Phases

`primary` | `secondary` | `resolved` | `cancelled`. Primary buys happen on a
bonding curve (`primary_order_usdc`), graduation moves a market to `secondary`.
Basket-eligible markets must be in `primary` (checked at quote time by Panta too).

## Position value estimation (per Panta docs)

- Open market: `value ≈ shares × sidePrice` (mark-to-market, display only).
- Resolved winner (`side === outcome`): ≈ **1 USDC per share**; use `winningShares`
  from claim build.
- Resolved loser: ≈ 0.

## On-chain program (verified 11 October 2026 from mainnet transactions)

- **Program id (mainnet):** `6gM5afTQBq5VZCfgpGqcsqzfWd5maLSCKWtGjbEobZMp` (upgradeable
  BPF program; not published in Panta's docs — found in sonar-panta, confirmed on chain).
  No legacy Anchor IDL account exists, but it is an Anchor program (8-byte
  `sha256("global:<ix>")` discriminators, `account:<Name>` account discriminators).
- **`primary_order_usdc`**: data = disc `2e89447431590df7` + `side: u8` (0 = YES, 1 = NO)
  + `amount: u64` (USDC base units). **No slippage / min-shares argument on chain** —
  slippage is only enforced by the API at build time.
  Accounts (12):

  | # | Account | Notes |
  |---|---|---|
  | 0 | buyer | signer, writable; pays position rent |
  | 1 | `Event` (the market) | writable, 2982 bytes |
  | 2 | `MarketConfig` | readonly, shared (`8mJjfx7S…`) |
  | 3 | market vault authority | readonly, 9-byte program account; owns the market's USDC vault |
  | 4 | market USDC vault | writable; ATA of #3 |
  | 5 | `UserPosition` | writable; PDA `["position", event, buyer]`, 202 bytes, buyer pubkey at offset 41 |
  | 6 | USDC mint | |
  | 7 | buyer USDC account | must be the buyer's **standard ATA** |
  | 8 | protocol fee USDC account | `9hwZLD2J…` (owner `4M2EtWmj…`) |
  | 9–11 | Token, ATA, System programs | |

- **Shares are not SPL tokens.** A buy only moves USDC (amount → market vault, 2% fee on
  top → fee account); shares are recorded in the buyer's `UserPosition` and logged as
  `Primary Order (USDC): side=…, amount=…, yes_price=…, no_price=…, minted=…` (prices 1e9-scaled).
- Panta's build adds an `spl-memo` instruction to purchases.
- `Event.resolved_at` (i64 unix) at offset 72: 0 while open, set on resolution (verified on 3
  resolved vs 3 open markets). The outcome byte is not mapped yet; `yes_price` is NOT pinned
  to 0/1 after resolution.
- `MarketConfig` is the PDA `["market_config"]`; the market vault authority is a `TokenVault`
  account (9 bytes: discriminator + bump) whose seeds are unknown — read it from a past buy.
- `Event` timestamps (i64 unix): 40 = end of the primary ("flash acquisition") window,
  48 = market end, 56 = expected resolution, 64 = created. Primary windows are short
  (hours to ~2 days); outside them buys fail with `FlashAcquisitionEnded` (6085).
- `claim_win_usdc`: data = disc `2ba06a33a74c141f` + claimer pubkey. Accounts: claimer
  (signer, w), MarketConfig, Event, market vault authority, market vault (w),
  `UserPosition`, `WinClaim` PDA `["win_claim", event, claimer]` (w, created), claimer's
  USDC ATA (w), USDC mint, Token, ATA, System.

### Spike: program-owned buyer (tokenized baskets) — PASSED

On a local validator cloning the mainnet program and a live market's accounts, a probe
program called `primary_order_usdc` via `invoke_signed` with a **PDA as buyer**
(system-owned, no data, holding SOL for rent) and the PDA's **off-curve USDC ATA** as #7.
Result: success; 1.02 USDC left the vault ATA, `UserPosition` was created with
`user = PDA`, ~74k CU per buy (4 legs ≈ 300k CU). So a basket vault program can hold
Panta positions itself. Still to verify: `claim_win_usdc` via CPI on a resolved market.

## Open items to verify with real calls (need an API key)

- [ ] Exact pagination cursor format for `/markets/` (opaque string in `nextCursor`).
- [ ] Whether one transaction may carry multiple primary_order instructions
  (spike S1) — determines single-tx baskets.
- [ ] Whether `/primaryordersubmit/` accepts a shared signature for multiple
  order ids (spike S3).
- [ ] Real creator/oracle fields exposure for the Resolution Center (spike S5).
- [ ] Exact program ids + top-level instruction shapes in `/primaryorderbuild/` and
  `/claim/build/` responses. Our safety check (`solana_tx::validate`) only allows
  Panta programs, ATA create, SPL Memo, our ComputeBudget pair and our fee `transfer_checked`.
  Panta-supplied ComputeBudget instructions are dropped at assembly (we set our
  own). If Panta returns System or Token instructions, the build is rejected —
  record what it returns and widen the allowlist deliberately.
