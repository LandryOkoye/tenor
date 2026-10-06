//! Market queries: upsert from Panta, browse with filters, snapshots.
//!
//! The `markets` table is a local cache of Panta's catalog (brief D10): the
//! worker keeps it fresh; reads never hit Panta directly (except the
//! bootstrap sync when the table is completely empty).

use chrono::{DateTime, TimeZone, Utc};
use tokio_postgres::Client;

use crate::domain::MarketSummary;

/// Convert unix seconds (Panta's format) -> DateTime, None for 0/absent.
fn unix_to_ts(secs: Option<i64>) -> Option<DateTime<Utc>> {
    secs.and_then(|s| Utc.timestamp_opt(s, 0).single())
}

/// Insert or update one market row from a Panta catalog row.
/// Keeps the first_seen_at of existing rows; everything else is refreshed.
pub async fn upsert_market(
    client: &Client,
    m: &crate::panta::MarketRow,
) -> Result<(), tokio_postgres::Error> {
    // Panta's volume is a human-readable string ("1200.00") -> base units.
    let volume_base = m
        .volume_usdc
        .as_deref()
        .and_then(|v| crate::panta::usdc_human_to_base(v).ok());

    client
        .execute(
            "INSERT INTO markets (id, title, category, phase, status, close_time,
                                  resolve_time, resolved_at, outcome, creator, oracle,
                                  rules_text, image_url, raw_json, volume, yes_price, no_price,
                                  updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                     now())
             ON CONFLICT (id) DO UPDATE SET
                title = EXCLUDED.title,
                category = EXCLUDED.category,
                phase = EXCLUDED.phase,
                status = EXCLUDED.status,
                close_time = EXCLUDED.close_time,
                resolve_time = EXCLUDED.resolve_time,
                resolved_at = EXCLUDED.resolved_at,
                outcome = EXCLUDED.outcome,
                creator = EXCLUDED.creator,
                oracle = EXCLUDED.oracle,
                rules_text = EXCLUDED.rules_text,
                image_url = EXCLUDED.image_url,
                raw_json = EXCLUDED.raw_json,
                volume = EXCLUDED.volume,
                yes_price = EXCLUDED.yes_price,
                no_price = EXCLUDED.no_price,
                updated_at = now()",
            &[
                &m.market_id,
                &m.title,
                &m.category,
                &m.phase,
                &m.status,
                &unix_to_ts(m.end_time),
                &unix_to_ts(m.resolution_time),
                // "resolved_at": best available signal is resolution_time when resolved.
                &unix_to_ts(if m.resolved { m.resolution_time } else { None }),
                &m.status.clone().map(|_| {
                    // outcome: derive yes/no from filled prices once resolved
                    match (&m.yes_price, m.resolved) {
                        (Some(p), true) if p == "1.00" || p == "1" => Some("yes".to_string()),
                        (Some(_), true) => Some("no".to_string()),
                        _ => None,
                    }
                }),
                // Panta catalog rows do not expose creator/oracle yet (spike S5);
                // we record what we have and enrich later.
                &Option::<String>::None,
                &Option::<String>::None,
                &m.description,
                &m.images.first().cloned(),
                &serde_json::to_value(m).unwrap_or(serde_json::Value::Null),
                &volume_base,
                &m.yes_price,
                &m.no_price,
            ],
        )
        .await?;
    Ok(())
}

/// A market row as stored locally (subset of columns we serve).
pub struct MarketRecord {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub phase: Option<String>,
    pub status: String,
    pub close_time: Option<DateTime<Utc>>,
    pub resolve_time: Option<DateTime<Utc>>,
    pub yes_price: Option<String>,
    pub no_price: Option<String>,
    pub volume: Option<i64>,
    pub image_url: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Read one market row by Panta market id.
pub async fn get_market(
    client: &Client,
    market_id: &str,
) -> Result<Option<MarketRecord>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, title, category, phase, status, close_time, resolve_time,
                    yes_price, no_price, volume, image_url, updated_at
             FROM markets WHERE id = $1",
            &[&market_id],
        )
        .await?;
    Ok(row.map(|r| MarketRecord {
        id: r.get(0),
        title: r.get(1),
        category: r.get(2),
        phase: r.get(3),
        status: r.get(4),
        close_time: r.get(5),
        resolve_time: r.get(6),
        yes_price: r.get(7),
        no_price: r.get(8),
        volume: r.get(9),
        image_url: r.get(10),
        updated_at: r.get(11),
    }))
}

/// Are we serving a stale copy? (Panta unreachable recently -> honest UI.)
fn is_stale(updated_at: DateTime<Utc>) -> bool {
    Utc::now() - updated_at > chrono::Duration::minutes(10)
}

impl MarketRecord {
    /// Local row -> API view. Prices come from the last snapshot-ish fields;
    /// `stale` flags rows not refreshed in the last 10 minutes (brief §12).
    pub fn to_summary(&self) -> MarketSummary {
        MarketSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            category: self.category.clone(),
            phase: self.phase.clone(),
            status: self.status.clone(),
            close_time: self.close_time,
            resolve_time: self.resolve_time,
            yes_price: self.yes_price.clone(),
            no_price: self.no_price.clone(),
            volume_usdc: self.volume.map(|v| v.to_string()),
            image_url: self.image_url.clone(),
            stale: is_stale(self.updated_at),
        }
    }
}

/// Browse markets: search, category filter, sort, offset-cursor pagination.
/// `sort`: volume (default) | newest | closing_soon. Returns (items, next_offset).
pub async fn list_markets(
    client: &Client,
    q: Option<&str>,
    category: Option<&str>,
    sort: Option<&str>,
    cursor: Option<usize>,
    limit: usize,
) -> Result<(Vec<MarketSummary>, Option<usize>), tokio_postgres::Error> {
    let offset = cursor.unwrap_or(0);

    // Sort whitelist — never interpolate user input into SQL.
    let order = match sort {
        Some("newest") => "first_seen_at DESC",
        Some("closing_soon") => "close_time ASC NULLS LAST",
        _ => "volume DESC NULLS LAST", // default: most volume first
    };
    let sql = format!(
        "SELECT id, title, category, phase, status, close_time, resolve_time,
                yes_price, no_price, volume, image_url, updated_at
         FROM markets
         WHERE ($1::text IS NULL OR title ILIKE '%' || $1 || '%')
           AND ($2::text IS NULL OR category = $2)
         ORDER BY {order}
         LIMIT $3 OFFSET $4"
    );

    let rows = client
        .query(&sql, &[&q, &category, &(limit as i64), &(offset as i64)])
        .await?;

    let items: Vec<MarketSummary> = rows
        .iter()
        .map(|r| {
            MarketRecord {
                id: r.get(0),
                title: r.get(1),
                category: r.get(2),
                phase: r.get(3),
                status: r.get(4),
                close_time: r.get(5),
                resolve_time: r.get(6),
                yes_price: r.get(7),
                no_price: r.get(8),
                volume: r.get(9),
                image_url: r.get(10),
                updated_at: r.get(11),
            }
            .to_summary()
        })
        .collect();

    // Cursor: report the next offset only when this page was full.
    let next = if items.len() == limit {
        Some(offset + items.len())
    } else {
        None
    };
    Ok((items, next))
}

/// How many markets do we have locally? (Used to decide bootstrap sync.)
pub async fn count(client: &Client) -> Result<i64, tokio_postgres::Error> {
    let row = client
        .query_one("SELECT count(*) FROM markets", &[])
        .await?;
    Ok(row.get(0))
}

/// Store one price snapshot (worker writes these every few minutes).
pub async fn insert_snapshot(
    client: &Client,
    market_id: &str,
    yes_price: Option<f64>,
    no_price: Option<f64>,
    volume: Option<i64>,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO market_snapshots (market_id, yes_price, no_price, volume)
             VALUES ($1, $2, $3, $4)",
            &[&market_id, &yes_price, &no_price, &volume],
        )
        .await?;
    Ok(())
}

/// Price history since we started tracking (D10): snapshots, oldest first.
pub async fn list_snapshots(
    client: &Client,
    market_id: &str,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<crate::domain::PricePoint>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT ts, yes_price, no_price FROM market_snapshots
             WHERE market_id = $1 AND ($2::timestamptz IS NULL OR ts >= $2)
             ORDER BY ts ASC",
            &[&market_id, &since],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| crate::domain::PricePoint {
            ts: r.get(0),
            yes_price: format_price(r.get(1)),
            no_price: format_price(r.get(2)),
        })
        .collect())
}

/// f64 price -> 2-decimal display string; None stays None.
fn format_price(p: Option<f64>) -> String {
    match p {
        Some(v) => format!("{v:.4}"),
        None => "0".to_string(),
    }
}

/// Published baskets a market appears in (shown on the market detail page).
pub async fn baskets_containing(
    client: &Client,
    market_id: &str,
) -> Result<Vec<crate::domain::BasketRef>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT t.slug, t.title
             FROM basket_legs l JOIN basket_templates t ON t.id = l.template_id
             WHERE l.market_id = $1 AND t.status = 'published'",
            &[&market_id],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| crate::domain::BasketRef {
            slug: r.get(0),
            title: r.get(1),
        })
        .collect())
}

/// Full detail row for one market (summary + rules/creator/oracle + baskets).
pub async fn market_detail(
    client: &Client,
    market_id: &str,
) -> Result<Option<crate::domain::MarketDetail>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, title, category, phase, status, close_time, resolve_time,
                    yes_price, no_price, volume, image_url, updated_at,
                    rules_text, creator, oracle
             FROM markets WHERE id = $1",
            &[&market_id],
        )
        .await?;
    let Some(row) = row else { return Ok(None) };

    let summary = MarketRecord {
        id: row.get(0),
        title: row.get(1),
        category: row.get(2),
        phase: row.get(3),
        status: row.get(4),
        close_time: row.get(5),
        resolve_time: row.get(6),
        yes_price: row.get(7),
        no_price: row.get(8),
        volume: row.get(9),
        image_url: row.get(10),
        updated_at: row.get(11),
    };

    Ok(Some(crate::domain::MarketDetail {
        id: summary.id.clone(),
        title: summary.title.clone(),
        category: summary.category.clone(),
        phase: summary.phase.clone(),
        status: summary.status.clone(),
        close_time: summary.close_time,
        resolve_time: summary.resolve_time,
        yes_price: summary.yes_price.clone(),
        no_price: summary.no_price.clone(),
        volume_usdc: summary.volume.map(|v| v.to_string()),
        image_url: summary.image_url.clone(),
        stale: is_stale(summary.updated_at),
        rules_text: row.get(12),
        creator: row.get(13),
        oracle: row.get(14),
        in_baskets: baskets_containing(client, market_id).await?,
    }))
}
