use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Standard error envelope for every non-2xx response (brief §9).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub error: ApiErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiErrorBody {
    /// Machine-readable code, e.g. `QUOTE_STALE`, `LEG_NOT_TRADABLE`,
    /// `SIMULATION_FAILED`, `TX_TOO_LARGE`, `SLIPPAGE_EXCEEDED`.
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ApiErrorBody {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        }
    }
}

/// Cursor-paginated list.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Page<T: TS> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Health {
    pub status: String,
    pub version: String,
}

/// `GET /readyz`: reachability of hard dependencies.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Ready {
    pub db: bool,
    pub rpc: bool,
    pub panta: bool,
}

/// Where a piece of returned data came from (brief §8: label APP analysis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DataSource {
    Panta,
    AppAnalysis,
}
