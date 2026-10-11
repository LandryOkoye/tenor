//! Resolution Center queries (brief §8). Everything computed here is
//! "Tenor analysis" — derived from our own snapshots and recorded events,
//! clearly separable from Panta's official data.

use chrono::{DateTime, Utc};
use tokio_postgres::Client;

/// Markets bucketed by resolution state (brief §8 status lists).
/// status: "resolving_soon" | "resolved" | "delayed"
pub struct ResolutionRow {
    pub market_id: String,
    pub title: String,
    pub close_time: Option<DateTime<Utc>>,
    pub expected_resolve_time: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub outcome: Option<String>,
    pub creator: Option<String>,
    /// Winning-side price from our last snapshot before resolution.
    pub last_price_before_resolution: Option<f64>,
}

/// Rows for a status bucket. `status` must be one of the three known values
/// (whitelisted below — never interpolated raw into SQL).
pub async fn markets_by_status(
    client: &Client,
    status: &str,
    limit: i64,
) -> Result<Vec<ResolutionRow>, tokio_postgres::Error> {
    // Whitelist bucket -> SQL predicate (user input never enters the SQL).
    let predicate = match status {
        "resolving_soon" => {
            "m.phase <> 'resolved' AND m.resolved = false
                             AND m.resolve_time <= now() + interval '48 hours'"
        }
        "resolved" => "m.resolved = true",
        "delayed" => {
            "m.phase <> 'resolved' AND m.resolved = false
                     AND m.resolve_time < now()"
        }
        _ => "false", // unknown bucket -> empty result, never an error
    };
    let sql = format!(
        "SELECT m.id, m.title, m.close_time, m.resolve_time, m.resolved_at, m.outcome,
                m.creator,
                (SELECT s.yes_price::float8 FROM market_snapshots s
                 WHERE s.market_id = m.id AND s.ts <= COALESCE(m.resolved_at, now())
                 ORDER BY s.ts DESC LIMIT 1) AS last_yes,
                (SELECT s.no_price::float8 FROM market_snapshots s
                 WHERE s.market_id = m.id AND s.ts <= COALESCE(m.resolved_at, now())
                 ORDER BY s.ts DESC LIMIT 1) AS last_no
         FROM markets m
         WHERE {predicate}
         ORDER BY m.resolve_time ASC NULLS LAST
         LIMIT $1"
    );

    let rows = client.query(&sql, &[&limit]).await?;
    Ok(rows
        .iter()
        .map(|r| {
            // The "last price" is the winning side's price: yes if the market
            // resolved yes (or is still open and we show the YES side), else no.
            let outcome: Option<String> = r.get(5);
            let last_yes: Option<f64> = r.get(7);
            let last_no: Option<f64> = r.get(8);
            let last_price = if outcome.as_deref() == Some("no") {
                last_no.or(last_yes)
            } else {
                last_yes.or(last_no)
            };
            ResolutionRow {
                market_id: r.get(0),
                title: r.get(1),
                close_time: r.get(2),
                expected_resolve_time: r.get(3),
                resolved_at: r.get(4),
                outcome,
                creator: r.get(6),
                last_price_before_resolution: last_price,
            }
        })
        .collect())
}

/// Full resolution detail for one market (rules, oracle, timeline).
pub struct MarketResolution {
    pub market_id: String,
    pub title: String,
    pub rules_text: Option<String>,
    pub oracle: Option<String>,
    pub creator: Option<String>,
    pub close_time: Option<DateTime<Utc>>,
    pub resolve_time: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub outcome: Option<String>,
    /// (ts, event_type, payload) in chronological order.
    pub timeline: Vec<(DateTime<Utc>, String, serde_json::Value)>,
}

pub async fn market_resolution(
    client: &Client,
    market_id: &str,
) -> Result<Option<MarketResolution>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, title, rules_text, oracle, creator, close_time, resolve_time,
                    resolved_at, outcome
             FROM markets WHERE id = $1",
            &[&market_id],
        )
        .await?;
    let Some(row) = row else { return Ok(None) };

    // The evidence timeline: everything we recorded, oldest first.
    let events = client
        .query(
            "SELECT ts, event_type, payload FROM market_events
             WHERE market_id = $1 ORDER BY ts ASC",
            &[&market_id],
        )
        .await?;
    let timeline = events
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect();

    Ok(Some(MarketResolution {
        market_id: row.get(0),
        title: row.get(1),
        rules_text: row.get(2),
        oracle: row.get(3),
        creator: row.get(4),
        close_time: row.get(5),
        resolve_time: row.get(6),
        resolved_at: row.get(7),
        outcome: row.get(8),
        timeline,
    }))
}

/// Creator scorecard (brief §8) — pure SQL aggregation over our markets table.
/// Returns None when the creator has no markets in our catalog.
pub async fn creator_scorecard(
    client: &Client,
    creator: &str,
) -> Result<Option<crate::domain::CreatorScorecard>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT
                count(*)                                                AS markets_created,
                count(*) FILTER (WHERE m.resolved)                       AS markets_resolved,
                -- % resolved within resolve_time + 24h of the expected time
                CASE WHEN count(*) FILTER (WHERE m.resolved) = 0 THEN NULL ELSE
                    count(*) FILTER (WHERE m.resolved AND m.resolved_at
                        <= m.resolve_time + interval '24 hours')
                   ::float8 / count(*) FILTER (WHERE m.resolved)
                END                                                       AS pct_on_time,
                -- median delay in hours (only resolved-late rows count)
                percentile_cont(0.5) WITHIN GROUP (ORDER BY
                    GREATEST(0, EXTRACT(EPOCH FROM (m.resolved_at - m.resolve_time)) / 3600.0))
                    FILTER (WHERE m.resolved)               AS median_delay_hours,
                -- surprise resolutions: last YES snapshot >= 0.90 but resolved NO
                count(*) FILTER (WHERE m.resolved AND m.outcome = 'no'
                    AND (SELECT s.yes_price::float8 FROM market_snapshots s
                         WHERE s.market_id = m.id AND s.ts <= m.resolved_at
                         ORDER BY s.ts DESC LIMIT 1) >= 0.90)  AS surprises,
                COALESCE(sum(m.volume), 0)                     AS total_volume
             FROM markets m
             WHERE m.creator = $1",
            &[&creator],
        )
        .await?;
    let Some(row) = row else { return Ok(None) };
    let created: i64 = row.get("markets_created");
    if created == 0 {
        return Ok(None);
    }

    // Format floats as clean decimal strings for the API (no f64 in JSON money).
    let fmt = |v: Option<f64>| v.map(|f| format!("{f:.2}"));
    Ok(Some(crate::domain::CreatorScorecard {
        creator: creator.to_string(),
        markets_created: created,
        markets_resolved: row.get("markets_resolved"),
        pct_resolved_on_time: fmt(row.get("pct_on_time")),
        median_delay_hours: fmt(row.get("median_delay_hours")),
        surprise_resolutions: row.get("surprises"),
        total_volume_usdc: row.get::<_, i64>("total_volume").to_string(),
        source: crate::domain::DataSource::AppAnalysis,
    }))
}

/// All markets of one creator (creator page listing).
pub async fn creator_markets(
    client: &Client,
    creator: &str,
) -> Result<Vec<ResolutionRow>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT m.id, m.title, m.close_time, m.resolve_time, m.resolved_at, m.outcome,
                    m.creator, NULL::float8 AS last_price
             FROM markets m
             WHERE m.creator = $1
             ORDER BY m.first_seen_at DESC",
            &[&creator],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| ResolutionRow {
            market_id: r.get(0),
            title: r.get(1),
            close_time: r.get(2),
            expected_resolve_time: r.get(3),
            resolved_at: r.get(4),
            outcome: r.get(5),
            creator: r.get(6),
            last_price_before_resolution: r.get(7),
        })
        .collect())
}
