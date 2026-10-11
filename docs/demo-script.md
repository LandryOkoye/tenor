# Demo video script (≈3 minutes)

Record with `./scripts/dev-mock.sh` running (seeded baskets) and the frontend pointed at it;
the vault segment uses `onchain/e2e/run.sh` in a terminal. Read numbers off the screen.

| Time | Screen | Say |
|---|---|---|
| 0:00 | Home: featured baskets | "Prediction markets are single bets or all-or-nothing parlays. Tenor sells baskets: one view, several markets, one signature — and every leg pays on its own." |
| 0:20 | Basket page "AFCON 2027: Super Eagles" | Legs, weights, live prices, index price. "Powered by Panta" on the buy panel. |
| 0:40 | Buy panel → quote | Per-leg breakdown: allocation, shares, Panta fee, Tenor fee. Notice: "You cannot sell before these markets resolve." |
| 1:00 | Wallet prompt (one signature) | "All legs and our fee are in ONE transaction: either the whole basket fills or nothing does. Before your wallet sees it we validate every instruction and simulate it; we only accept back the exact bytes we built." |
| 1:20 | Portfolio | Basket with per-leg status, value, P&L; claim all. |
| 1:40 | Resolution Center | Resolving soon / delayed; market timeline; creator scorecard labelled "Tenor analysis". |
| 2:00 | Terminal: `onchain/e2e/run.sh` | "Panta has no sell. So we made baskets a token. Our Anchor vault buys each leg on Panta through CPI — we proved a program can be a Panta buyer — and mints a basket token at NAV." Show `create_basket` ok, deposit log with `PrimaryOrderUsdc` and minted tokens. |
| 2:30 | README "How we use Panta" | "We mapped Panta's program — accounts, PDAs, claims — and wrote it all up for Panta's team." |
| 2:50 | Logo | "Tenor. Invest in a view. Powered by Panta." |
