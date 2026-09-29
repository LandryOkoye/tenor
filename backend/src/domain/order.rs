use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::domain::market::Side;

/// `POST /v1/orders/quote`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrderQuoteRequest {
    pub market_id: String,
    pub side: Side,
    /// USDC base units, integer string.
    pub amount_usdc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrderQuote {
    pub quote_id: String,
    pub market_id: String,
    pub side: Side,
    pub amount_usdc: String,
    pub expected_shares: String,
    /// Decimal string 0..=1.
    pub avg_price: String,
    pub panta_fee_usdc: String,
    pub our_fee_usdc: String,
    pub price_impact_bps: Option<i32>,
    pub expires_at: DateTime<Utc>,
}

/// Human-readable disclosure shown before signing (brief §12).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TxSummary {
    pub summary_lines: Vec<String>,
    pub total_usdc: String,
    pub our_fee_usdc: String,
    pub panta_fees_usdc: String,
    /// Warnings, e.g. "You cannot sell before these markets resolve".
    pub notices: Vec<String>,
}

/// `POST /v1/orders/{quoteId}/build` response.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BuildResponse {
    pub order_id: Uuid,
    /// Base64-encoded unsigned VersionedTransactions (usually exactly one).
    pub transactions: Vec<String>,
    pub summary: TxSummary,
    pub expires_at: DateTime<Utc>,
}

/// `POST /v1/orders/{orderId}/submit` and basket purchase submit.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SubmitRequest {
    /// Base64-encoded signed VersionedTransactions, same order as build returned.
    pub signed_txs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OrderStatus {
    Quoted,
    Built,
    Submitted,
    Confirmed,
    Filled,
    Failed,
}

/// `GET /v1/orders/{orderId}`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrderView {
    pub order_id: Uuid,
    pub market_id: String,
    pub market_title: Option<String>,
    pub side: Side,
    pub amount_usdc: String,
    pub expected_shares: Option<String>,
    pub status: OrderStatus,
    pub tx_signature: Option<String>,
    pub error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
