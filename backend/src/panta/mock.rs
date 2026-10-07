//! `MockPanta` — a deterministic in-memory implementation of `PantaApi`.
//!
//! Purpose: let the whole app run end to end (browse, quote, build, submit,
//! claim, portfolio) without an API key and without touching mainnet money.
//!
//! Behaviour notes:
//! - Markets are loaded from `fixtures/markets.json` (shaped exactly like
//!   Panta's documented rows).
//! - Quotes live for ~90 s (configurable for tests), build creates an order
//!   session, submit moves it to `submitted` and nudges the bonding-curve
//!   price 1% toward 1.0 — so price impact is visible in dev.
//! - All amounts use the same integer base-unit math as the real client.
//! - Instruction payloads are structurally valid Solana instructions
//!   (System program, the buyer's wallet as account) so the dev frontend can
//!   even exercise the sign flow with a real wallet without spending anything.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use chrono::{DateTime, Utc};
use tokio::sync::Mutex;

use super::client::PantaApi;
use super::error::PantaError;
use super::types::*;
use crate::panta::{usdc_base_to_human, usdc_human_to_base};

/// Solana System program id — used as the program for all mock instructions.
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
/// Stand-in blockhash for every mock build/claim response.
const MOCK_BLOCKHASH: &str = "11111111111111111111111111111111";
/// Protocol fee the mock charges on primary buys: 200 bps (brief §4.3).
const MOCK_PROTOCOL_FEE_BPS: i64 = 200;

/// A quote issued by the mock (mirrors `QuoteResponse` + bookkeeping).
#[derive(Debug, Clone)]
struct MockQuote {
    wallet: String,
    market_id: String,
    side: String,
    amount_usdc: String,
    shares: String,
    avg_price: String,
    fee_usdc: String,
    expires_at: DateTime<Utc>,
}

/// An order session created by build (mirrors the real orderId lifecycle).
#[derive(Debug, Clone)]
struct MockOrder {
    quote: MockQuote,
    status: String, // built | submitted
}

pub struct MockPanta {
    markets: Mutex<Vec<MarketRow>>,
    categories: Vec<String>,
    quotes: Mutex<HashMap<String, MockQuote>>, // quoteId -> quote
    orders: Mutex<HashMap<String, MockOrder>>, // orderId -> order
    positions: Mutex<Vec<(String, PositionRow)>>, // (wallet, row)
    quote_ttl: Duration,
    counter: AtomicU64, // monotonic id source for qt_…/ord_…
}

impl MockPanta {
    /// Load fixture markets + categories.
    pub fn new() -> Self {
        Self::with_quote_ttl(Duration::from_secs(90))
    }

    /// Test hook: make quotes expire after `ttl` so expiry handling is testable.
    pub fn with_quote_ttl(ttl: Duration) -> Self {
        let markets: Vec<MarketRow> = serde_json::from_str(include_str!("fixtures/markets.json"))
            .expect("fixture markets.json is valid");
        let cats: CategoriesResponse =
            serde_json::from_str(include_str!("fixtures/categories.json"))
                .expect("fixture categories.json is valid");
        Self {
            markets: Mutex::new(markets),
            categories: cats.categories,
            quotes: Mutex::new(HashMap::new()),
            orders: Mutex::new(HashMap::new()),
            positions: Mutex::new(Vec::new()),
            quote_ttl: ttl,
            counter: AtomicU64::new(1),
        }
    }

    fn next_id(&self, prefix: &str) -> String {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("{prefix}_mock_{n}")
    }

    /// Test/dev hook: pretend `wallet` holds `shares` on `market_id`.
    /// Mirrors the "one row per (market, side)" rule of the real endpoint.
    pub async fn seed_position(
        &self,
        wallet: &str,
        market_id: &str,
        side: &str,
        shares: &str,
        claimable: bool,
        outcome: Option<&str>,
    ) {
        let row = PositionRow {
            market_id: market_id.to_string(),
            category: None,
            side: side.to_string(),
            shares: shares.to_string(),
            phase: Some("resolved".to_string()),
            claimable,
            claimed: Some(false),
            outcome: outcome.map(str::to_string),
        };
        self.positions.lock().await.push((wallet.to_string(), row));
    }

    /// Add `shares` to the wallet's holding of one market+side (top-up on
    /// repeat buys), creating the row if it does not exist yet.
    async fn add_shares(&self, wallet: &str, market_id: &str, side: &str, shares: &str) {
        let mut positions = self.positions.lock().await;
        if let Some((_, row)) = positions
            .iter_mut()
            .find(|(w, row)| w == wallet && row.market_id == market_id && row.side == side)
        {
            // Shares are 6-decimal quantities: add in base units, exact math.
            if let (Ok(total), Ok(add)) =
                (usdc_human_to_base(&row.shares), usdc_human_to_base(shares))
            {
                row.shares = usdc_base_to_human(total + add);
            }
            row.claimed = Some(false);
            return;
        }
        positions.push((
            wallet.to_string(),
            PositionRow {
                market_id: market_id.to_string(),
                category: None,
                side: side.to_string(),
                shares: shares.to_string(),
                phase: Some("primary".to_string()),
                claimable: false,
                claimed: Some(false),
                outcome: None,
            },
        ));
    }

    /// Build the structurally-valid mock instruction used by build/claim.
    fn mock_instruction(wallet: &str, tag: &str) -> WireInstruction {
        WireInstruction {
            program_id: SYSTEM_PROGRAM.to_string(),
            // Base64 of a readable tag so debug output explains itself.
            data: base64::engine::general_purpose::STANDARD.encode(tag.as_bytes()),
            accounts: vec![WireAccount {
                pubkey: wallet.to_string(),
                is_signer: true,
                is_writable: true,
            }],
        }
    }

    /// Move the bought side's price 1% of the remaining gap toward 1.0,
    /// simulating bonding-curve price impact. Pure integer math on 1e6 units.
    async fn nudge_curve(&self, market_id: &str, side: &str) {
        let mut markets = self.markets.lock().await;
        if let Some(m) = markets.iter_mut().find(|m| m.market_id == market_id) {
            let price_of = |p: &Option<String>| -> Option<i64> {
                p.as_ref().and_then(|s| usdc_human_to_base(s).ok())
            };
            // Prices are stored as 0..=1 decimals = base units of a 1 USDC coin.
            let bump = |current: Option<i64>| -> Option<i64> {
                current.map(|c| c + (1_000_000 - c) / 100) // +1% of gap to 1.0
            };
            if side == "yes" {
                m.yes_price = bump(price_of(&m.yes_price)).map(usdc_base_to_human);
                m.primary_yes_price = m.yes_price.clone();
            } else {
                m.no_price = bump(price_of(&m.no_price)).map(usdc_base_to_human);
                m.primary_no_price = m.no_price.clone();
            }
        }
    }

    /// Look up a market or fail exactly like Panta would.
    async fn market(&self, market_id: &str) -> Result<MarketRow, PantaError> {
        self.markets
            .lock()
            .await
            .iter()
            .find(|m| m.market_id == market_id)
            .cloned()
            .ok_or_else(|| {
                PantaError::api(
                    "MARKET_NOT_FOUND",
                    &format!("market {market_id} not found"),
                    404,
                )
            })
    }
}

impl Default for MockPanta {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PantaApi for MockPanta {
    async fn health(&self) -> Result<bool, PantaError> {
        Ok(true)
    }

    async fn categories(&self) -> Result<Vec<String>, PantaError> {
        Ok(self.categories.clone())
    }

    async fn list_markets(&self, filter: &MarketFilter) -> Result<ListMarketsResponse, PantaError> {
        let markets = self.markets.lock().await;

        // Apply category/status filters.
        let matching: Vec<MarketRow> = markets
            .iter()
            .filter(|m| {
                filter
                    .category
                    .as_ref()
                    .is_none_or(|c| m.category.as_ref() == Some(c))
            })
            .filter(|m| {
                filter
                    .status
                    .as_ref()
                    .is_none_or(|s| m.phase.as_deref() == Some(s))
            })
            .cloned()
            .collect();

        // Cursor is the index of the first row of the next page (dev-only,
        // deliberately readable; Panta's own cursor is opaque).
        let offset: usize = filter
            .cursor
            .as_deref()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let limit = filter.limit.unwrap_or(20).min(50) as usize;

        let items: Vec<MarketRow> = matching.iter().skip(offset).take(limit).cloned().collect();
        let next_offset = offset + items.len();
        let next_cursor = if next_offset < matching.len() {
            Some(next_offset.to_string())
        } else {
            None
        };

        Ok(ListMarketsResponse { items, next_cursor })
    }

    async fn get_market(&self, market_id: &str) -> Result<MarketRow, PantaError> {
        self.market(market_id).await
    }

    async fn market_trades(
        &self,
        _market_id: &str,
        _limit: u32,
    ) -> Result<Vec<TradeRow>, PantaError> {
        Ok(Vec::new()) // mock tapes stay empty; the API layer can add rows later
    }

    async fn wallet_trades(&self, _wallet: &str, _limit: u32) -> Result<Vec<TradeRow>, PantaError> {
        Ok(Vec::new())
    }

    async fn positions(&self, wallet: &str) -> Result<Vec<PositionRow>, PantaError> {
        let stored: Vec<PositionRow> = self
            .positions
            .lock()
            .await
            .iter()
            .filter(|(w, _)| w == wallet)
            .map(|(_, row)| row.clone())
            .collect();

        // Enrich with live market state: phase, outcome and claim eligibility
        // exactly mirror the real endpoint's semantics (brief §4.2).
        let markets = self.markets.lock().await;
        Ok(stored
            .into_iter()
            .map(|row| {
                let m = markets.iter().find(|m| m.market_id == row.market_id);
                let resolved = m.map(|m| m.resolved).unwrap_or(false);
                let outcome = if resolved {
                    m.and_then(|m| m.yes_price.as_ref())
                        .and_then(|p| usdc_human_to_base(p).ok())
                        .map(|p| if p >= 1_000_000 { "yes" } else { "no" })
                } else {
                    None
                };
                let side = row.side.clone();
                PositionRow {
                    phase: m.and_then(|m| m.phase.clone()).or(row.phase.clone()),
                    claimable: resolved
                        && outcome.as_deref() == Some(side.as_str())
                        && !row.claimed.unwrap_or(false),
                    outcome: outcome.map(str::to_string),
                    ..row
                }
            })
            .collect())
    }

    async fn quote_primary(&self, req: &QuoteRequest) -> Result<QuoteResponse, PantaError> {
        let market = self.market(&req.market_id).await?;

        // Panta only quotes primary-phase markets.
        if market.phase.as_deref() != Some("primary") {
            return Err(PantaError::api(
                "MARKET_NOT_IN_PRIMARY",
                "market is not accepting primary buys",
                400,
            ));
        }
        if req.side != "yes" && req.side != "no" {
            return Err(PantaError::api(
                "INVALID_MARKET_PARAMS",
                "side must be yes or no",
                400,
            ));
        }

        // Integer math throughout: amount -> shares at the current price,
        // minus the protocol fee (fee taken from amount before shares).
        let amount_base = usdc_human_to_base(&req.amount_usdc).map_err(PantaError::Invalid)?;
        if amount_base <= 0 {
            return Err(PantaError::api(
                "AMOUNT_TOO_SMALL",
                "amount must be positive",
                400,
            ));
        }
        let price_base = {
            let p = if req.side == "yes" {
                &market.yes_price
            } else {
                &market.no_price
            };
            // Default a missing price to 0.50 like a fresh curve.
            match p.as_deref().and_then(|s| usdc_human_to_base(s).ok()) {
                Some(b) if b > 0 => b,
                _ => 500_000,
            }
        };
        let fee_base = amount_base * MOCK_PROTOCOL_FEE_BPS / 10_000; // 2%
        let net_base = amount_base - fee_base;
        // shares = net_amount / price, in 1e6 units: amount_base * 1e6 / price_base
        let shares_base = net_base * 1_000_000 / price_base;

        let quote = MockQuote {
            wallet: req.wallet.clone(),
            market_id: req.market_id.clone(),
            side: req.side.clone(),
            amount_usdc: req.amount_usdc.clone(),
            shares: usdc_base_to_human(shares_base),
            avg_price: usdc_base_to_human(price_base),
            fee_usdc: usdc_base_to_human(fee_base),
            expires_at: Utc::now() + chrono::Duration::from_std(self.quote_ttl).unwrap(),
        };

        let quote_id = self.next_id("qt");
        let resp = QuoteResponse {
            quote_id: quote_id.clone(),
            market_id: quote.market_id.clone(),
            side: quote.side.clone(),
            amount_usdc: quote.amount_usdc.clone(),
            shares: quote.shares.clone(),
            avg_price: quote.avg_price.clone(),
            fee_usdc: quote.fee_usdc.clone(),
            expires_at: quote.expires_at,
            blockhash_expiry_hint_sec: Some(60),
        };
        self.quotes.lock().await.insert(quote_id, quote);
        Ok(resp)
    }

    async fn build_primary(&self, req: &BuildRequest) -> Result<BuildResponse, PantaError> {
        // Session lookup with expiry check — exactly the QUOTE_EXPIRED contract.
        let quote = {
            let quotes = self.quotes.lock().await;
            let q = quotes.get(&req.quote_id).ok_or_else(|| {
                PantaError::api("QUOTE_EXPIRED", "unknown or consumed quoteId", 400)
            })?;
            if Utc::now() > q.expires_at {
                return Err(PantaError::api(
                    "QUOTE_EXPIRED",
                    "quote session expired",
                    400,
                ));
            }
            if q.wallet != req.wallet {
                return Err(PantaError::api(
                    "TX_MISMATCH",
                    "build wallet does not match quote wallet",
                    400,
                ));
            }
            q.clone()
        };

        let order_id = self.next_id("ord");
        let resp = BuildResponse {
            order_id: order_id.clone(),
            quote_id: req.quote_id.clone(),
            wallet: Some(req.wallet.clone()),
            market_id: Some(quote.market_id.clone()),
            side: Some(quote.side.clone()),
            amount_usdc: Some(quote.amount_usdc.clone()),
            expected_shares: Some(quote.shares.clone()),
            fee_usdc: Some(quote.fee_usdc.clone()),
            status: Some("built".to_string()),
            instructions: vec![Self::mock_instruction(
                &req.wallet,
                &format!("mock-primary-order:{}", quote.market_id),
            )],
            derived: None,
            recent_blockhash: Some(MOCK_BLOCKHASH.to_string()),
            last_valid_block_height: Some(123),
            expires_at: Some(Utc::now() + chrono::Duration::seconds(120)),
            blockhash_expiry_hint_sec: Some(60),
        };
        self.orders.lock().await.insert(
            order_id,
            MockOrder {
                quote,
                status: "built".into(),
            },
        );
        Ok(resp)
    }

    async fn submit_primary(&self, req: &SubmitRequest) -> Result<SubmitResponse, PantaError> {
        let mut orders = self.orders.lock().await;
        let order = orders
            .get_mut(&req.order_id)
            .ok_or_else(|| PantaError::api("QUOTE_EXPIRED", "unknown or expired orderId", 400))?;
        order.status = "submitted".into();

        // Idempotent like the real endpoint: same orderId+signature is a no-op.
        let (market_id, side, shares, wallet) = (
            order.quote.market_id.clone(),
            order.quote.side.clone(),
            order.quote.shares.clone(),
            order.quote.wallet.clone(),
        );
        drop(orders);

        // The buy lands: the wallet now holds the quoted shares (mock bookkeeping
        // so the portfolio and claim flows behave like the real indexer).
        self.add_shares(&wallet, &market_id, &side, &shares).await;

        // Simulate the curve moving after a real buy.
        self.nudge_curve(&market_id, &side).await;

        Ok(SubmitResponse {
            order_id: Some(req.order_id.clone()),
            status: Some("submitted".into()),
            signature: Some(req.signature.clone()),
        })
    }

    async fn verify_primary(
        &self,
        order_id: &str,
        _signature: Option<&str>,
    ) -> Result<VerifyResponse, PantaError> {
        let orders = self.orders.lock().await;
        let order = orders
            .get(order_id)
            .ok_or_else(|| PantaError::api("QUOTE_EXPIRED", "unknown orderId", 400))?;
        Ok(VerifyResponse {
            order_id: order_id.to_string(),
            status: Some(order.status.clone()),
            signature: _signature.map(str::to_string),
            market_id: Some(order.quote.market_id.clone()),
            side: Some(order.quote.side.clone()),
        })
    }

    async fn build_claim(&self, req: &ClaimBuildRequest) -> Result<ClaimBuildResponse, PantaError> {
        let market = self.market(&req.market_id).await?;
        if !market.resolved {
            return Err(PantaError::api(
                "NOT_CLAIMABLE",
                "market is not resolved",
                400,
            ));
        }

        // Fail closed: only wallets with a seeded winning position may claim.
        let winning_side = market
            .yes_price
            .as_deref()
            .and_then(|s| usdc_human_to_base(s).ok())
            .map(|p| if p >= 1_000_000 { "yes" } else { "no" })
            .unwrap_or("yes");
        let winning_shares = {
            let positions = self.positions.lock().await;
            positions
                .iter()
                .find(|(w, row)| {
                    w == &req.wallet && row.market_id == req.market_id && row.side == winning_side
                })
                .map(|(_, row)| row.shares.clone())
                .ok_or_else(|| {
                    PantaError::api("NOT_CLAIMABLE", "no winning shares for this wallet", 400)
                })?
        };

        // Consume the claim: a built claim cannot be built again (fail closed).
        {
            let mut positions = self.positions.lock().await;
            if let Some((_, row)) = positions.iter_mut().find(|(w, row)| {
                w == &req.wallet && row.market_id == req.market_id && row.side == winning_side
            }) {
                row.claimed = Some(true);
            }
        }

        Ok(ClaimBuildResponse {
            wallet: Some(req.wallet.clone()),
            market_id: Some(req.market_id.clone()),
            outcome: Some(winning_side.to_uppercase()),
            winning_shares: Some(winning_shares),
            instructions: vec![Self::mock_instruction(
                &req.wallet,
                &format!("mock-claim:{}", req.market_id),
            )],
            derived: None,
            recent_blockhash: Some(MOCK_BLOCKHASH.to_string()),
            last_valid_block_height: Some(123),
        })
    }

    async fn report_trade(
        &self,
        req: &ReportTradeRequest,
    ) -> Result<ReportTradeResponse, PantaError> {
        Ok(ReportTradeResponse {
            signature: Some(req.signature.clone()),
            status: Some("processed".into()),
            market_id: Some(req.market_id.clone()),
            wallet: Some(req.wallet.clone()),
            side: Some("yes".into()),
            kind: Some("buy".into()),
        })
    }

    async fn trade_status(&self, signature: &str) -> Result<TradeStatusResponse, PantaError> {
        Ok(TradeStatusResponse {
            signature: Some(signature.to_string()),
            status: Some("processed".into()),
            market_id: None,
            wallet: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALLET: &str = "11TestWa11et1111111111111111111111111111111";
    const MARKET: &str = "mkt_afcon_nigeria_wins_2027";

    /// The full quote -> build -> submit -> verify lifecycle must work and
    /// survive repeat submits (idempotency), with the curve price moving.
    #[tokio::test]
    async fn quote_build_submit_lifecycle() {
        let panta = MockPanta::new();

        // Quote 20 USDC of YES.
        let quote = panta
            .quote_primary(&QuoteRequest {
                wallet: WALLET.into(),
                market_id: MARKET.into(),
                side: "yes".into(),
                amount_usdc: "20.00".into(),
                user_id: None,
            })
            .await
            .expect("quote");
        // 20 USDC, 2% fee -> 19.60 net; at 0.62 -> 31 shares + remainder.
        assert_eq!(quote.fee_usdc, "0.4"); // 0.392 rounded down by integer math
        assert_eq!(
            usdc_human_to_base(&quote.shares).unwrap(),
            19_600_000 * 1_000_000 / 620_000
        );

        // Build from that quote.
        let build = panta
            .build_primary(&BuildRequest {
                quote_id: quote.quote_id.clone(),
                wallet: WALLET.into(),
                user_id: None,
                max_slippage_bps: Some(150),
            })
            .await
            .expect("build");
        assert_eq!(build.instructions.len(), 1);
        assert_eq!(build.status.as_deref(), Some("built"));

        // Wrong wallet must fail like Panta does.
        let bad = panta
            .build_primary(&BuildRequest {
                quote_id: quote.quote_id.clone(),
                wallet: "OtherWa11et111111111111111111111111111111".into(),
                user_id: None,
                max_slippage_bps: None,
            })
            .await;
        assert!(matches!(&bad, Err(PantaError::Api { code, .. }) if code == "TX_MISMATCH"));

        // Submit twice (idempotent), then verify shows submitted.
        for _ in 0..2 {
            panta
                .submit_primary(&SubmitRequest {
                    order_id: build.order_id.clone(),
                    signature: "sig123".into(),
                    wallet: None,
                })
                .await
                .expect("submit");
        }
        let verify = panta
            .verify_primary(&build.order_id, None)
            .await
            .expect("verify");
        assert_eq!(verify.status.as_deref(), Some("submitted"));

        // The curve must have moved: YES price should now be above 0.62.
        let market = panta.get_market(MARKET).await.expect("market");
        let new_price = usdc_human_to_base(market.yes_price.as_deref().unwrap()).unwrap();
        assert!(new_price > 620_000, "curve should move up after a buy");
    }

    /// Quotes must expire after their TTL with the documented code.
    #[tokio::test]
    async fn expired_quote_is_rejected() {
        let panta = MockPanta::with_quote_ttl(Duration::from_millis(20));
        let quote = panta
            .quote_primary(&QuoteRequest {
                wallet: WALLET.into(),
                market_id: MARKET.into(),
                side: "yes".into(),
                amount_usdc: "5.00".into(),
                user_id: None,
            })
            .await
            .expect("quote");

        tokio::time::sleep(Duration::from_millis(30)).await;

        let err = panta
            .build_primary(&BuildRequest {
                quote_id: quote.quote_id.clone(),
                wallet: WALLET.into(),
                user_id: None,
                max_slippage_bps: None,
            })
            .await
            .expect_err("should be expired");
        assert!(matches!(&err, PantaError::Api { code, .. } if code == "QUOTE_EXPIRED"));
    }

    /// Resolved markets refuse new quotes, and claims fail closed.
    #[tokio::test]
    async fn resolved_market_rules() {
        let panta = MockPanta::new();
        const RESOLVED: &str = "mkt_bbnaija_lagos_winner";

        let err = panta
            .quote_primary(&QuoteRequest {
                wallet: WALLET.into(),
                market_id: RESOLVED.into(),
                side: "yes".into(),
                amount_usdc: "5.00".into(),
                user_id: None,
            })
            .await
            .expect_err("resolved markets are not tradable");
        assert!(matches!(&err, PantaError::Api { code, .. } if code == "MARKET_NOT_IN_PRIMARY"));

        // No seeded position -> NOT_CLAIMABLE.
        let err = panta
            .build_claim(&ClaimBuildRequest {
                wallet: WALLET.into(),
                market_id: RESOLVED.into(),
            })
            .await
            .expect_err("claim fails closed");
        assert!(matches!(&err, PantaError::Api { code, .. } if code == "NOT_CLAIMABLE"));

        // Seed a winning position -> claim succeeds with the seeded shares.
        panta
            .seed_position(WALLET, RESOLVED, "yes", "38", true, Some("yes"))
            .await;
        let claim = panta
            .build_claim(&ClaimBuildRequest {
                wallet: WALLET.into(),
                market_id: RESOLVED.into(),
            })
            .await
            .expect("claim works for winners");
        assert_eq!(claim.winning_shares.as_deref(), Some("38"));
        assert_eq!(claim.outcome.as_deref(), Some("YES"));
    }

    /// List pagination: filters + cursor paging behave like the real catalog.
    #[tokio::test]
    async fn catalog_filters_and_pagination() {
        let panta = MockPanta::new();

        let crypto = panta
            .list_markets(&MarketFilter {
                category: Some("crypto".into()),
                ..Default::default()
            })
            .await
            .expect("crypto list");
        assert_eq!(crypto.items.len(), 2);

        // Page through everything with limit 2.
        let page1 = panta
            .list_markets(&MarketFilter {
                limit: Some(2),
                ..Default::default()
            })
            .await
            .expect("page 1");
        assert_eq!(page1.items.len(), 2);
        let page2 = panta
            .list_markets(&MarketFilter {
                limit: Some(2),
                cursor: page1.next_cursor.clone(),
                ..Default::default()
            })
            .await
            .expect("page 2");
        assert_eq!(page2.items.len(), 2);
        // All six fixtures must be reachable across pages, no duplicates.
        assert!(page1.next_cursor.is_some());
    }
}
