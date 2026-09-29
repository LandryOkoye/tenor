use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::domain::market::Side;

/// A single-market position (not part of a recorded basket purchase).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct PortfolioPosition {
    pub market_id: String,
    pub market_title: String,
    pub side: Side,
    pub shares: String,
    /// Decimal string 0..=1, when the market is still pricing.
    pub current_price: Option<String>,
    /// shares × side price, or shares × 1 USDC for resolved wins.
    pub value_usdc: String,
    pub cost_basis_usdc: Option<String>,
    pub unrealized_pnl_usdc: Option<String>,
    pub resolved: bool,
    pub won: Option<bool>,
    pub claimable: bool,
}

/// A basket as shown in the portfolio (brief §7.6).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct PortfolioBasket {
    pub purchase_id: Uuid,
    pub template_slug: String,
    pub template_title: String,
    pub status: crate::domain::basket::PurchaseStatus,
    pub cost_basis_usdc: String,
    pub current_value_usdc: String,
    pub unrealized_pnl_usdc: String,
    pub legs_resolved: i32,
    pub legs_total: i32,
    pub legs: Vec<PortfolioBasketLeg>,
    /// True if the wallet's on-chain shares no longer match our records —
    /// the user moved/altered positions outside the app ("basket modified outside APP").
    pub modified_outside: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct PortfolioBasketLeg {
    pub order_id: Uuid,
    pub market_id: String,
    pub market_title: String,
    pub side: Side,
    pub shares: Option<String>,
    pub status: crate::domain::basket::LegStatus,
    pub value_usdc: String,
    pub claimable: bool,
}

/// `GET /v1/portfolio`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct Portfolio {
    pub wallet: String,
    pub single_positions: Vec<PortfolioPosition>,
    pub baskets: Vec<PortfolioBasket>,
    pub total_value_usdc: String,
    pub total_cost_basis_usdc: String,
    pub total_unrealized_pnl_usdc: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub enum ActivityKind {
    Buy,
    BasketBuy,
    Claim,
}

/// `GET /v1/portfolio/activity`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct ActivityItem {
    pub kind: ActivityKind,
    pub market_id: Option<String>,
    pub market_title: Option<String>,
    pub basket_purchase_id: Option<Uuid>,
    pub side: Option<Side>,
    pub amount_usdc: String,
    pub tx_signature: Option<String>,
    pub ts: DateTime<Utc>,
}

/// `POST /v1/claims/build`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct ClaimsBuildRequest {
    /// Markets to claim on. Empty/one entry = single claim; all claimable = claim all.
    pub market_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct ClaimsBuildResponse {
    pub transactions: Vec<String>,
    pub summary: crate::domain::order::TxSummary,
    pub total_claimable_usdc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct ClaimsSubmitResponse {
    pub signatures: Vec<String>,
    pub status: String,
}
