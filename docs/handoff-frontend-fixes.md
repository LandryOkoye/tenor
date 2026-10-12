# Frontend fixes needed (from backend review, 12 Oct 2026)

Four issues found while preparing the demo. Each has a known cause; fix in `frontend/`.

## 1. Portfolio asks the user to sign in again (most important)

**Cause:** a render-order race. `wallet/session.tsx` restores the session from `localStorage`
correctly, but attaches the token with `setAuthToken` inside a `useEffect` (line ~31). React runs
child effects before parent effects, so the Portfolio query (`api/queries.ts:45`, enabled as soon as
`session` exists) fires **without** the `Authorization` header. The API answers 401, the 401 handler
(`setUnauthorizedHandler(signOut)`) wipes the saved session, and the user sees "Sign in" again.

**Fix:** attach the token synchronously, before any child can query — e.g. call
`setAuthToken(current?.token ?? null)` during render (or in the state initializer when restoring
from storage), or gate authed queries on "token attached" rather than "session exists". Keep the
401 → sign-out behaviour for genuinely expired tokens.

(One-off, expected: a session saved against the local API is rejected by the deployed API — different
`JWT_SECRET` — so users sign in once per environment.)

## 2. Single-market buy: button labels don't match

`features/markets/BuyPanel.tsx:167` says **"Buy YES · 10 USDC"**, then the review step
`features/trade/SignReview.tsx:88` says **"Sign in wallet"** — which reads like logging in. Use one
consistent action, e.g. panel "Review buy · YES · 10 USDC" → review "Approve in wallet" /
"Buy YES · 10 USDC", and match the basket flow's wording.

## 3. Resolution tabs are not exposed as buttons

`components/ui/primitives.tsx` `Tabs` renders `<button role="tab">`; `role="tab"` replaces the button
role, so tools and screen readers looking for buttons don't find them, and the tab pattern is only
half-implemented (no arrow-key navigation, no `aria-controls`/`tabpanel`). Either drop `role="tab"` /
`role="tablist"` (plain toggle buttons with `aria-pressed`), or implement the full WAI-ARIA tabs
pattern. Add `type="button"`.

## 4. Copy: don't say "live"

Tenor is not launched and the demo runs on recorded demo data, so avoid "live" claims:
- `features/baskets/BasketBuyPanel.tsx:218` — "Your basket is live." → e.g. "All legs filled."
- `features/landing/LandingPage.tsx:130` — "Live P&L" → "Real-time P&L" or "P&L"
- `features/baskets/BasketsPage.tsx:70` — "curated from live markets" → "curated from open markets"

## Reminder: API types

`frontend/src/api/generated/` is generated from the Rust backend and committed (Vercel has no Rust
toolchain). Never hand-edit it; after backend type changes run `./scripts/typegen.sh` and commit.
