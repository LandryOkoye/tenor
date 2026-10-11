use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::domain::common::DataSource;

/// Resolution Center buckets for `GET /v1/resolution/markets?status=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ResolutionStatus {
    Open,
    ResolvingSoon,
    Resolved,
    Delayed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResolutionMarketRow {
    pub market_id: String,
    pub title: String,
    pub status: ResolutionStatus,
    pub close_time: Option<DateTime<Utc>>,
    pub expected_resolve_time: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    /// "yes" | "no" once final, when known.
    pub outcome: Option<String>,
    /// Winning-side decimal price right before resolution, when we have it.
    pub last_price_before_resolution: Option<String>,
    pub creator: Option<String>,
    pub source: DataSource,
}

/// One step in a market's evidence timeline (brief §8).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TimelineEvent {
    pub ts: DateTime<Utc>,
    /// `created` | `trading_started` | `closed` | `resolution_proposed` | `resolved` | `claims_open`
    pub event_type: String,
    pub label: String,
    /// Solana explorer URL when an on-chain reference exists.
    pub explorer_url: Option<String>,
    pub source: DataSource,
}

/// Creator / oracle track record (brief §8). Every field is Tenor analysis.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreatorScorecard {
    pub creator: String,
    pub markets_created: i64,
    pub markets_resolved: i64,
    /// Decimal string 0..=1, e.g. "0.87".
    pub pct_resolved_on_time: Option<String>,
    pub median_delay_hours: Option<String>,
    /// Resolved against a last price ≥ 0.90.
    pub surprise_resolutions: i64,
    pub total_volume_usdc: String,
    pub source: DataSource,
}

/// `GET /v1/resolution/markets/{id}`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResolutionMarketDetail {
    pub market_id: String,
    pub title: String,
    pub status: ResolutionStatus,
    pub rules_text: Option<String>,
    pub oracle: Option<String>,
    pub creator: Option<String>,
    pub expected_resolve_time: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub outcome: Option<String>,
    pub last_price_before_resolution: Option<String>,
    pub timeline: Vec<TimelineEvent>,
    pub creator_scorecard: Option<CreatorScorecard>,
}

/// `GET /v1/resolution/creators/{creator}`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreatorDetail {
    pub scorecard: CreatorScorecard,
    pub markets: Vec<ResolutionMarketRow>,
}
