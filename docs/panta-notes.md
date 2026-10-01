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

## Open items to verify with real calls (need an API key)

- [ ] Exact pagination cursor format for `/markets/` (opaque string in `nextCursor`).
- [ ] Whether one transaction may carry multiple primary_order instructions
  (spike S1) — determines single-tx baskets.
- [ ] Whether `/primaryordersubmit/` accepts a shared signature for multiple
  order ids (spike S3).
- [ ] Real creator/oracle fields exposure for the Resolution Center (spike S5).
