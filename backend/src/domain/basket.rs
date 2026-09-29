use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::domain::market::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub enum BasketStatus {
    Draft,
    Published,
    Closed,
    Settled,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketTemplateSummary {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub cover_url: Option<String>,
    pub status: BasketStatus,
    pub leg_count: i32,
    pub min_buy_usdc: String,
    pub max_buy_usdc: String,
    /// Decimal string: weighted average of leg prices ("index price").
    pub index_price: Option<String>,
    /// Seconds until the earliest leg closes, when known.
    pub closes_in_seconds: Option<i64>,
    /// Total USDC base units bought through this template.
    pub total_bought_usdc: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketLegView {
    pub market_id: String,
    pub market_title: String,
    pub side: Side,
    pub weight_bps: i32,
    /// Current spot price of the chosen side, decimal string.
    pub current_price: Option<String>,
    pub close_time: Option<DateTime<Utc>>,
    pub tradable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketTemplateDetail {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub cover_url: Option<String>,
    pub status: BasketStatus,
    pub min_buy_usdc: String,
    pub max_buy_usdc: String,
    pub index_price: Option<String>,
    pub legs: Vec<BasketLegView>,
    pub total_bought_usdc: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
}

/// `POST /v1/baskets/{id}/quote`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketQuoteRequest {
    /// Total USDC base units to spend (including our fee).
    pub amount_usdc: String,
    pub slippage_bps: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketQuoteLeg {
    pub market_id: String,
    pub market_title: String,
    pub side: Side,
    pub weight_bps: i32,
    pub allocation_usdc: String,
    pub expected_shares: String,
    pub avg_price: String,
    pub panta_fee_usdc: String,
    pub price_impact_bps: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketQuote {
    pub quote_id: Uuid,
    pub template_id: Uuid,
    pub amount_usdc: String,
    pub our_fee_usdc: String,
    pub total_panta_fees_usdc: String,
    pub legs: Vec<BasketQuoteLeg>,
    pub expires_at: DateTime<Utc>,
}

/// `POST /v1/baskets/quotes/{quoteId}/build`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketBuildRequest {
    pub wallet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketBuildResponse {
    pub purchase_id: Uuid,
    pub transactions: Vec<String>,
    pub summary: crate::domain::order::TxSummary,
    pub expires_at: DateTime<Utc>,
}

/// Purchase state machine (brief §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub enum PurchaseStatus {
    Quoted,
    Built,
    Signed,
    Submitted,
    Confirmed,
    Filled,
    Partial,
    Failed,
    Active,
    PartiallyResolved,
    Resolved,
    Claimed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub enum LegStatus {
    Pending,
    Filled,
    Failed,
    ResolvedWin,
    ResolvedLoss,
    Claimed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct PurchaseLegView {
    pub order_id: Uuid,
    pub market_id: String,
    pub market_title: String,
    pub side: Side,
    pub amount_usdc: String,
    pub shares: Option<String>,
    pub status: LegStatus,
    pub tx_signature: Option<String>,
    pub error_code: Option<String>,
    pub current_price: Option<String>,
    pub claimable: bool,
}

/// `GET /v1/baskets/purchases/{id}`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketPurchaseView {
    pub id: Uuid,
    pub template_id: Uuid,
    pub template_slug: String,
    pub template_title: String,
    pub amount_usdc: String,
    pub fee_usdc: String,
    pub status: PurchaseStatus,
    pub legs: Vec<PurchaseLegView>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
