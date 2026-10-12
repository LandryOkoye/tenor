//! Resolution Center endpoints (brief §8, Phase 2 v1 per the build plan).
//!
//! Shows how every market resolves: rules, oracle, evidence timeline, and
//! creator track records. Everything we compute is labelled "Tenor analysis";
//! everything from Panta's catalog is labelled "panta" — never mixed.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::collections::HashMap;

use crate::domain::{
    CreatorDetail, DataSource, ResolutionMarketDetail, ResolutionMarketRow, ResolutionStatus,
    TimelineEvent,
};
use crate::error::AppError;
use crate::queries;
use crate::state::AppState;

/// `GET /v1/resolution/markets?status=resolving_soon|resolved|delayed`
pub async fn markets(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<ResolutionMarketRow>>, AppError> {
    let status = params
        .get("status")
        .map(String::as_str)
        .unwrap_or("resolving_soon");
    // Unknown buckets return empty, not an error (whitelisted in the query).
    let bucket = match status {
        "resolved" | "delayed" => status,
        _ => "resolving_soon",
    };
    let status_enum = match bucket {
        "resolved" => ResolutionStatus::Resolved,
        "delayed" => ResolutionStatus::Delayed,
        _ => ResolutionStatus::ResolvingSoon,
    };

    let client = state.db.get().await?;
    let rows = queries::resolution::markets_by_status(&client, bucket, 50).await?;

    Ok(Json(
        rows.into_iter()
            .map(|r| ResolutionMarketRow {
                market_id: r.market_id,
                title: r.title,
                status: status_enum,
                close_time: r.close_time,
                expected_resolve_time: r.expected_resolve_time,
                resolved_at: r.resolved_at,
                outcome: r.outcome,
                last_price_before_resolution: r
                    .last_price_before_resolution
                    .map(|p| format!("{p:.2}")),
                creator: r.creator,
                // Rows come from Panta's catalog through our sync.
                source: DataSource::Panta,
            })
            .collect(),
    ))
}

/// `GET /v1/resolution/markets/{id}` — one market's full resolution page.
pub async fn detail(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> Result<Json<ResolutionMarketDetail>, AppError> {
    let client = state.db.get().await?;
    let m = queries::resolution::market_resolution(&client, &market_id)
        .await?
        .ok_or_else(|| AppError::not_found("market not found"))?;

    // Status bucket for display (same rules as the listing endpoint).
    let now = chrono::Utc::now();
    let status = if m.resolved_at.is_some() {
        ResolutionStatus::Resolved
    } else if m.resolve_time.map(|t| t < now).unwrap_or(false) {
        ResolutionStatus::Delayed
    } else {
        ResolutionStatus::ResolvingSoon
    };

    // Evidence timeline (brief §8): our recorded events, plus synthetic
    // entries from catalog timestamps so even never-synced markets have a
    // basic story ("trading started", "expected resolution").
    let mut timeline: Vec<TimelineEvent> = Vec::new();
    if let Some(t) = m.start_time {
        timeline.push(TimelineEvent {
            ts: t,
            event_type: "trading_started".into(),
            label: "Trading opened".into(),
            explorer_url: None,
            source: DataSource::Panta,
        });
    }
    if let Some(t) = m.close_time {
        timeline.push(TimelineEvent {
            ts: t,
            event_type: "closed".into(),
            label: "Trading closed".into(),
            explorer_url: None,
            source: DataSource::Panta,
        });
    }
    if let Some(t) = m.resolve_time {
        timeline.push(TimelineEvent {
            ts: t,
            event_type: "resolution_expected".into(),
            label: if m.resolved_at.is_some_and(|r| r > t) {
                "Resolution was due".into()
            } else {
                "Resolution expected".into()
            },
            explorer_url: None,
            source: DataSource::Panta,
        });
    }
    for (ts, event_type, payload) in &m.timeline {
        // Panta's resolution timestamp is shown below; our own record of
        // when we first saw it resolved would only duplicate it.
        if event_type == "resolved" && m.resolved_at.is_some() {
            continue;
        }
        // Events with a timestamp BEFORE the synthetic ones stay in order:
        // we sort the whole timeline below.
        timeline.push(TimelineEvent {
            ts: *ts,
            event_type: event_type.clone(),
            label: humanize_event(event_type, payload),
            explorer_url: None, // on-chain links added when Panta exposes sigs
            source: DataSource::AppAnalysis,
        });
    }
    if let Some(t) = m.resolved_at {
        timeline.push(TimelineEvent {
            ts: t,
            event_type: "resolved".into(),
            label: format!(
                "Resolved: {}",
                m.outcome
                    .as_deref()
                    .map(|o| o.to_uppercase())
                    .unwrap_or("?".into())
            ),
            explorer_url: None,
            source: DataSource::Panta,
        });
    }
    timeline.sort_by_key(|e| e.ts);

    // Creator scorecard when we know the creator (often null until spike S5).
    let scorecard = match &m.creator {
        Some(creator) => queries::resolution::creator_scorecard(&client, creator).await?,
        None => None,
    };

    Ok(Json(ResolutionMarketDetail {
        market_id: m.market_id,
        title: m.title,
        status,
        rules_text: m.rules_text,
        oracle: m.oracle,
        creator: m.creator,
        expected_resolve_time: m.resolve_time,
        resolved_at: m.resolved_at,
        outcome: m.outcome,
        last_price_before_resolution: None, // kept for the listing; detail focuses on the timeline
        timeline,
        creator_scorecard: scorecard,
    }))
}

/// `GET /v1/resolution/creators/{creator}` — scorecard + their markets.
pub async fn creator(
    State(state): State<AppState>,
    Path(creator): Path<String>,
) -> Result<Json<CreatorDetail>, AppError> {
    let client = state.db.get().await?;

    let scorecard = queries::resolution::creator_scorecard(&client, &creator)
        .await?
        .ok_or_else(|| AppError::not_found("creator not found in our catalog"))?;

    let rows = queries::resolution::creator_markets(&client, &creator).await?;
    let markets = rows
        .into_iter()
        .map(|r| {
            let status = if r.resolved_at.is_some() {
                ResolutionStatus::Resolved
            } else if r
                .expected_resolve_time
                .map(|t| t < chrono::Utc::now())
                .unwrap_or(false)
            {
                ResolutionStatus::Delayed
            } else {
                ResolutionStatus::ResolvingSoon
            };
            ResolutionMarketRow {
                market_id: r.market_id,
                title: r.title,
                status,
                close_time: r.close_time,
                expected_resolve_time: r.expected_resolve_time,
                resolved_at: r.resolved_at,
                outcome: r.outcome,
                last_price_before_resolution: r
                    .last_price_before_resolution
                    .map(|p| format!("{p:.2}")),
                creator: r.creator,
                source: DataSource::AppAnalysis,
            }
        })
        .collect();

    Ok(Json(CreatorDetail { scorecard, markets }))
}

/// Human-readable label for a recorded event (used in the timeline UI).
fn humanize_event(event_type: &str, payload: &serde_json::Value) -> String {
    match event_type {
        "created" => "First tracked by Tenor".to_string(),
        "status_change" => {
            let to = &payload["to"];
            match (to["phase"].as_str(), to["resolved"].as_bool()) {
                (_, Some(true)) | (Some("resolved"), _) => {
                    "Panta marked the market resolved".into()
                }
                (Some("secondary"), _) => "Graduated to secondary trading".into(),
                (Some("primary"), _) => "Primary trading open".into(),
                (Some("cancelled"), _) => "Market cancelled".into(),
                (Some(other), _) => format!("Market moved to the {other} phase"),
                (None, _) => "Market status changed".into(),
            }
        }
        "resolved" => format!(
            "Resolved: {}",
            payload
                .get("outcome")
                .and_then(|o| o.as_str())
                .map(|o| o.to_uppercase())
                .unwrap_or_else(|| "?".into())
        ),
        "basket_closed" => "Closed for basket purchases (leg no longer tradable)".to_string(),
        other => format!("Event: {other}"),
    }
}
