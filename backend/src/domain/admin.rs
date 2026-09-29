use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::domain::basket::BasketStatus;
use crate::domain::market::Side;

/// Light view of a Panta market for the admin curation UI.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct AdminPantaMarket {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub status: String,
    pub close_time: Option<DateTime<Utc>>,
    pub volume_usdc: Option<String>,
    pub yes_price: Option<String>,
    pub no_price: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct CreateBasketLeg {
    pub market_id: String,
    pub side: Side,
    pub weight_bps: i32,
}

/// `POST /v1/admin/baskets` (brief §7.2).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct CreateBasketRequest {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub cover_url: Option<String>,
    pub min_buy_usdc: String,
    pub max_buy_usdc: String,
    pub legs: Vec<CreateBasketLeg>,
}

/// `PATCH /v1/admin/baskets/{id}` — every field optional; `legs` replaces all legs.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct UpdateBasketRequest {
    pub slug: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub cover_url: Option<String>,
    pub min_buy_usdc: Option<String>,
    pub max_buy_usdc: Option<String>,
    pub legs: Option<Vec<CreateBasketLeg>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct AdminBasketLeg {
    pub market_id: String,
    pub side: Side,
    pub weight_bps: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../frontend/src/api/generated/")]
pub struct AdminBasketView {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub cover_url: Option<String>,
    pub status: BasketStatus,
    pub min_buy_usdc: String,
    pub max_buy_usdc: String,
    pub legs: Vec<AdminBasketLeg>,
    /// Validation warnings, e.g. thin liquidity or close time too soon (brief §7.9).
    pub warnings: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
}
