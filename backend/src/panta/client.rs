//! The `PantaApi` trait — one method per Panta endpoint we use — plus the
//! factory that picks the right implementation from configuration.
//!
//! Having the trait lets the rest of the app depend on an interface, not the
//! HTTP client, so tests and offline development can use `MockPanta`.

use std::sync::Arc;

use async_trait::async_trait;

use super::error::PantaError;
use super::http::HttpPanta;
use super::mock::MockPanta;
use super::types::*;
use crate::config::Config;

/// Every Panta capability the app needs. Implementations:
/// - `HttpPanta` — real API over HTTPS
/// - `MockPanta` — deterministic in-memory fake for dev/tests
#[async_trait]
pub trait PantaApi: Send + Sync {
    /// Cheap reachability probe used by `/readyz`.
    async fn health(&self) -> Result<bool, PantaError>;

    // -- catalog (read family) --
    async fn categories(&self) -> Result<Vec<String>, PantaError>;
    async fn list_markets(&self, filter: &MarketFilter) -> Result<ListMarketsResponse, PantaError>;
    async fn get_market(&self, market_id: &str) -> Result<MarketRow, PantaError>;
    async fn market_trades(&self, market_id: &str, limit: u32)
        -> Result<Vec<TradeRow>, PantaError>;
    async fn wallet_trades(&self, wallet: &str, limit: u32) -> Result<Vec<TradeRow>, PantaError>;

    // -- positions (positions family) --
    async fn positions(&self, wallet: &str) -> Result<Vec<PositionRow>, PantaError>;

    // -- primary buy flow (quote / build / register families) --
    async fn quote_primary(&self, req: &QuoteRequest) -> Result<QuoteResponse, PantaError>;
    async fn build_primary(&self, req: &BuildRequest) -> Result<BuildResponse, PantaError>;
    async fn submit_primary(&self, req: &SubmitRequest) -> Result<SubmitResponse, PantaError>;
    async fn verify_primary(
        &self,
        order_id: &str,
        signature: Option<&str>,
    ) -> Result<VerifyResponse, PantaError>;

    // -- claims (build family) --
    async fn build_claim(&self, req: &ClaimBuildRequest) -> Result<ClaimBuildResponse, PantaError>;

    // -- attribution (register family) --
    async fn report_trade(
        &self,
        req: &ReportTradeRequest,
    ) -> Result<ReportTradeResponse, PantaError>;
    async fn trade_status(&self, signature: &str) -> Result<TradeStatusResponse, PantaError>;
}

/// Build the right client from configuration.
///
/// `PANTA_MOCK=true` (default in development) -> `MockPanta`.
/// Otherwise -> `HttpPanta` pointing at the configured base URL with the
/// configured API key. `Config::validate` already refuses mock mode in
/// production, so mock data can never leak into a real deployment.
pub fn make_client(config: &Config) -> Arc<dyn PantaApi> {
    if config.panta_mock {
        tracing::warn!("PANTA_MOCK=true: serving deterministic mock Panta data");
        Arc::new(MockPanta::new())
    } else {
        if config.panta_api_key.is_empty() {
            tracing::warn!(
                "PANTA_MOCK=false but PANTA_API_KEY is empty — Panta will answer UNAUTHORIZED"
            );
        }
        Arc::new(HttpPanta::new(
            config.panta_api_base_url.clone(),
            config.panta_api_key.clone(),
        ))
    }
}
