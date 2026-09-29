use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub enum Side {
    Yes,
    No,
}

impl Side {
    pub fn opposite(self) -> Self {
        match self {
            Side::Yes => Side::No,
            Side::No => Side::Yes,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Yes => "yes",
            Side::No => "no",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct MarketSummary {
    /// Panta market id.
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub phase: Option<String>,
    pub status: String,
    pub close_time: Option<DateTime<Utc>>,
    pub resolve_time: Option<DateTime<Utc>>,
    /// Decimal string 0..=1, when the market is pricing.
    pub yes_price: Option<String>,
    pub no_price: Option<String>,
    /// Lifetime volume in USDC base units, if known.
    pub volume_usdc: Option<String>,
    pub image_url: Option<String>,
    /// True if this row is served from our cache while Panta was unreachable.
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct MarketDetail {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub phase: Option<String>,
    pub status: String,
    pub close_time: Option<DateTime<Utc>>,
    pub resolve_time: Option<DateTime<Utc>>,
    pub yes_price: Option<String>,
    pub no_price: Option<String>,
    pub volume_usdc: Option<String>,
    pub image_url: Option<String>,
    pub rules_text: Option<String>,
    pub creator: Option<String>,
    pub oracle: Option<String>,
    pub stale: bool,
    /// Published basket templates this market appears in.
    pub in_baskets: Vec<BasketRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct BasketRef {
    pub slug: String,
    pub title: String,
}

/// `GET /v1/markets/{id}/history` – one price snapshot (D10: our own snapshots).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct PricePoint {
    pub ts: DateTime<Utc>,
    pub yes_price: String,
    pub no_price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct MarketHistory {
    pub market_id: String,
    pub range: String,
    /// Empty until we have tracked the market; show "history since we started tracking".
    pub points: Vec<PricePoint>,
}

/// One public trade on a market (from Panta's trade tape).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct MarketTrade {
    pub signature: Option<String>,
    pub side: Option<Side>,
    pub amount_usdc: Option<String>,
    pub shares: Option<String>,
    pub price: Option<String>,
    pub ts: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct Category {
    pub id: String,
    pub name: String,
}

/// Query params for `GET /v1/markets`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct MarketsQuery {
    pub category: Option<String>,
    pub q: Option<String>,
    /// `volume` | `newest` | `closing_soon`
    pub sort: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
