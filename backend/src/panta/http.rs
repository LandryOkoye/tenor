//! `HttpPanta` — the real Panta client over HTTPS.
//!
//! Hard rules enforced here (brief §4, §21; docs/panta-notes.md):
//! - `X-Api-Key` on every request (never sent to the frontend).
//! - **Trailing slash on every path** — Panta rejects `/markets` without it.
//! - 15 s request timeout; every call has one (no exceptions).
//! - Retries with exponential backoff + jitter for 429 (honouring
//!   `Retry-After`), 5xx, timeouts and connection errors. Never retries
//!   4xx business errors — those are deterministic failures.
//! - One client-side rate limiter per Panta rate-limit family, sized at
//!   half of Panta's documented budget so we stay safely under it.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::de::DeserializeOwned;

use super::client::PantaApi;
use super::error::PantaError;
use super::rate::RateLimiter;
use super::types::*;

// --- Tunables ---------------------------------------------------------------

/// Total attempts per request: 1 try + 3 retries.
const MAX_ATTEMPTS: u32 = 4;
/// Per-request HTTP timeout. Panta has no SLA; we fail rather than hang.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// First backoff step; doubles every retry (250ms, 500ms, 1s) plus jitter.
const BACKOFF_BASE_MS: u64 = 250;

// --- Client ----------------------------------------------------------------

pub struct HttpPanta {
    http: reqwest::Client,
    /// Base URL without trailing slash, e.g. "https://live-api.panta.market/api/v1".
    base_url: String,
    api_key: String,
    // One limiter per documented Panta family (max/60s: read 120, positions 60,
    // quote 30, build 20, register 40). We use half of each as our own budget.
    lim_read: RateLimiter,
    lim_positions: RateLimiter,
    lim_quote: RateLimiter,
    lim_build: RateLimiter,
    lim_register: RateLimiter,
}

impl HttpPanta {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client"),
            // Strip a trailing slash so `base + "/markets/"` never doubles it.
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            lim_read: RateLimiter::new(60, Duration::from_secs(60)),
            lim_positions: RateLimiter::new(30, Duration::from_secs(60)),
            lim_quote: RateLimiter::new(15, Duration::from_secs(60)),
            lim_build: RateLimiter::new(10, Duration::from_secs(60)),
            lim_register: RateLimiter::new(20, Duration::from_secs(60)),
        }
    }

    /// Full URL for a path that already includes leading+trailing slashes.
    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path)
    }

    /// Core request loop with retries. `query` items are appended as URL
    /// params; `body` (if any) is sent as JSON.
    async fn send<T: DeserializeOwned>(
        &self,
        limiter: &RateLimiter,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&serde_json::Value>,
    ) -> Result<T, PantaError> {
        // The last 429 we saw (to report Retry-After if retries are exhausted).
        let mut last_retry_after: Option<u64> = None;

        for attempt in 0..MAX_ATTEMPTS {
            // Client-side budget first, so our own bursts queue instead of 429ing.
            limiter.acquire().await;

            let mut req = self
                .http
                .request(method.clone(), self.url(path))
                .header("X-Api-Key", &self.api_key);
            if !query.is_empty() {
                req = req.query(&query);
            }
            if let Some(json) = body {
                req = req.json(json);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();

                    // Success: decode the documented shape.
                    if status.is_success() {
                        return resp
                            .json::<T>()
                            .await
                            .map_err(|e| PantaError::Decode(e.to_string()));
                    }

                    // Rate limited: honour Retry-After, then retry.
                    if status.as_u16() == 429 {
                        let retry_after = resp
                            .headers()
                            .get("Retry-After")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok());
                        last_retry_after = retry_after;
                        sleep_backoff(attempt, retry_after).await;
                        continue;
                    }

                    // Server error: probably transient, retry.
                    if status.is_server_error() {
                        sleep_backoff(attempt, None).await;
                        continue;
                    }

                    // Other 4xx: a documented, deterministic error — do not retry.
                    let bytes = resp.bytes().await.unwrap_or_default();
                    return Err(decode_api_error(&bytes, status.as_u16()));
                }
                Err(e) if e.is_timeout() => {
                    sleep_backoff(attempt, None).await;
                }
                Err(e) if e.is_connect() || e.is_request() => {
                    // Transport-level problem; retry with backoff.
                    sleep_backoff(attempt, None).await;
                }
                Err(e) => return Err(PantaError::Network(e.to_string())),
            }
        }

        // Retries exhausted. Report what actually blocked us.
        Err(match last_retry_after {
            Some(ra) => PantaError::RateLimited {
                retry_after_secs: Some(ra),
            },
            None => PantaError::Timeout,
        })
    }

    /// GET helper (empty body).
    async fn get<T: DeserializeOwned>(
        &self,
        limiter: &RateLimiter,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, PantaError> {
        self.send(limiter, reqwest::Method::GET, path, query, None)
            .await
    }

    /// POST helper with a JSON body.
    async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        limiter: &RateLimiter,
        path: &str,
        body: &B,
    ) -> Result<T, PantaError> {
        // Serialize once; `send` re-sends the same bytes on retries
        // (idempotent enough for Panta's retry-safe endpoints).
        let json = serde_json::to_value(body)
            .map_err(|e| PantaError::Invalid(format!("could not serialize request: {e}")))?;
        self.send(limiter, reqwest::Method::POST, path, &[], Some(&json))
            .await
    }

    /// Build a small query string for `GET /markets/` filters.
    fn market_query(filter: &MarketFilter) -> Vec<(&'static str, String)> {
        let mut q = Vec::new();
        if let Some(c) = &filter.category {
            q.push(("category", c.clone()));
        }
        if let Some(s) = &filter.status {
            q.push(("status", s.clone()));
        }
        if let Some(c) = &filter.cursor {
            q.push(("cursor", c.clone()));
        }
        if let Some(l) = filter.limit {
            q.push(("limit", l.to_string()));
        }
        q
    }
}

/// Parse Panta's error envelope `{"code": …, "message": …}` from a body.
fn decode_api_error(bytes: &[u8], status: u16) -> PantaError {
    match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(v) => {
            let code = v
                .get("code")
                .and_then(|c| c.as_str())
                .unwrap_or("UNKNOWN")
                .to_string();
            let message = v
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown panta error")
                .to_string();
            PantaError::Api {
                code,
                message,
                status,
            }
        }
        Err(_) => PantaError::Api {
            code: "UNKNOWN".into(),
            message: format!("undecodable error body: {}", String::from_utf8_lossy(bytes)),
            status,
        },
    }
}

/// Exponential backoff + jitter; honours `Retry-After` when it is larger.
/// Jitter uses clock nanos — no extra dependency, plenty of entropy here.
async fn sleep_backoff(attempt: u32, retry_after_secs: Option<u64>) {
    let base = Duration::from_millis(BACKOFF_BASE_MS * (1 << attempt));
    let jitter = Duration::from_nanos(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 % 100_000_000) // 0..100ms
            .unwrap_or(0),
    );
    let mut wait = base + jitter;
    if let Some(secs) = retry_after_secs {
        wait = wait.max(Duration::from_secs(secs));
    }
    tokio::time::sleep(wait).await;
}

#[async_trait]
impl PantaApi for HttpPanta {
    async fn health(&self) -> Result<bool, PantaError> {
        match self.categories().await {
            Ok(_) => Ok(true),
            Err(e) => {
                tracing::warn!(error = %e, "panta health check failed");
                Ok(false)
            }
        }
    }

    async fn categories(&self) -> Result<Vec<String>, PantaError> {
        let resp: CategoriesResponse = self.get(&self.lim_read, "categories/", &[]).await?;
        Ok(resp.categories)
    }

    async fn list_markets(&self, filter: &MarketFilter) -> Result<ListMarketsResponse, PantaError> {
        self.get(&self.lim_read, "markets/", &Self::market_query(filter))
            .await
    }

    async fn get_market(&self, market_id: &str) -> Result<MarketRow, PantaError> {
        let path = format!("markets/{market_id}/"); // trailing slash required
        self.get(&self.lim_read, &path, &[]).await
    }

    async fn market_trades(
        &self,
        market_id: &str,
        limit: u32,
    ) -> Result<Vec<TradeRow>, PantaError> {
        let path = format!("markets/{market_id}/trades/");
        let resp: MarketTradesResponse = self
            .get(&self.lim_read, &path, &[("limit", limit.to_string())])
            .await?;
        Ok(resp.items)
    }

    async fn wallet_trades(&self, wallet: &str, limit: u32) -> Result<Vec<TradeRow>, PantaError> {
        let path = format!("wallets/{wallet}/trades/");
        let resp: WalletTradesResponse = self
            .get(&self.lim_read, &path, &[("limit", limit.to_string())])
            .await?;
        Ok(resp.items)
    }

    async fn positions(&self, wallet: &str) -> Result<Vec<PositionRow>, PantaError> {
        let resp: PositionsResponse = self
            .get(
                &self.lim_positions,
                "positions/",
                &[("wallet", wallet.to_string())],
            )
            .await?;
        Ok(resp.positions)
    }

    async fn quote_primary(&self, req: &QuoteRequest) -> Result<QuoteResponse, PantaError> {
        if req.market_id.is_empty() || req.wallet.is_empty() {
            return Err(PantaError::Invalid(
                "wallet and marketId are required".into(),
            ));
        }
        self.post(&self.lim_quote, "primaryorderquote/", req).await
    }

    async fn build_primary(&self, req: &BuildRequest) -> Result<BuildResponse, PantaError> {
        if req.quote_id.is_empty() || req.wallet.is_empty() {
            return Err(PantaError::Invalid(
                "quoteId and wallet are required".into(),
            ));
        }
        self.post(&self.lim_build, "primaryorderbuild/", req).await
    }

    async fn submit_primary(&self, req: &SubmitRequest) -> Result<SubmitResponse, PantaError> {
        if req.order_id.is_empty() || req.signature.is_empty() {
            return Err(PantaError::Invalid(
                "orderId and signature are required".into(),
            ));
        }
        self.post(&self.lim_register, "primaryordersubmit/", req)
            .await
    }

    async fn verify_primary(
        &self,
        order_id: &str,
        signature: Option<&str>,
    ) -> Result<VerifyResponse, PantaError> {
        if order_id.is_empty() {
            return Err(PantaError::Invalid("orderId is required".into()));
        }
        let body = VerifyRequest {
            order_id: order_id.to_string(),
            signature: signature.map(str::to_string),
            wallet: None,
        };
        self.post(&self.lim_register, "primaryorderverify/", &body)
            .await
    }

    async fn build_claim(&self, req: &ClaimBuildRequest) -> Result<ClaimBuildResponse, PantaError> {
        if req.market_id.is_empty() || req.wallet.is_empty() {
            return Err(PantaError::Invalid(
                "wallet and marketId are required".into(),
            ));
        }
        self.post(&self.lim_build, "claim/build/", req).await
    }

    async fn report_trade(
        &self,
        req: &ReportTradeRequest,
    ) -> Result<ReportTradeResponse, PantaError> {
        if req.signature.is_empty() || req.wallet.is_empty() || req.market_id.is_empty() {
            return Err(PantaError::Invalid(
                "signature, wallet and marketId are required".into(),
            ));
        }
        self.post(&self.lim_register, "trades/", req).await
    }

    async fn trade_status(&self, signature: &str) -> Result<TradeStatusResponse, PantaError> {
        if signature.is_empty() {
            return Err(PantaError::Invalid("signature is required".into()));
        }
        let path = format!("trades/{signature}/");
        self.get(&self.lim_read, &path, &[]).await
    }
}
