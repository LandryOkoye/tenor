//! Market browsing endpoints (brief §9).
//!
//! Reads are served from OUR database (the worker keeps it in sync); if the
//! table is completely empty (fresh dev setup before the worker ever ran), the
//! list endpoint bootstraps itself by pulling the first catalog pages from
//! Panta once — so the app works end-to-end out of the box.

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{TimeZone, Utc};
use tokio::sync::Mutex;

use crate::domain::{
    Category, MarketDetail, MarketHistory, MarketSummary, MarketTrade, MarketsQuery, Page,
};
use crate::error::AppError;
use crate::queries;
use crate::state::AppState;

/// How long we cache the Panta categories allowlist in memory.
const CATEGORY_CACHE_TTL: chrono::Duration = chrono::Duration::minutes(5);

/// In-memory cache holder for the categories list.
static CATEGORY_CACHE: Mutex<Option<(std::time::Instant, Vec<String>)>> = Mutex::const_new(None);

/// `GET /v1/categories` — the allowlist of market categories (Panta data,
/// cached 5 minutes to be gentle with rate limits).
pub async fn categories(State(state): State<AppState>) -> Result<Json<Vec<Category>>, AppError> {
    let mut guard = CATEGORY_CACHE.lock().await;

    // Fresh enough? Serve from memory.
    if let Some((fetched_at, cats)) = guard.as_ref() {
        if fetched_at.elapsed() < CATEGORY_CACHE_TTL.to_std().unwrap() {
            return Ok(Json(
                cats.iter()
                    .map(|c| Category {
                        id: c.clone(),
                        name: c.clone(),
                    })
                    .collect(),
            ));
        }
    }

    // Refetch from Panta (or the mock).
    let cats = state.panta.categories().await?;
    *guard = Some((std::time::Instant::now(), cats.clone()));

    Ok(Json(
        cats.into_iter()
            .map(|c| Category {
                id: c.clone(),
                name: c,
            })
            .collect(),
    ))
}

/// Pull up to N pages of 50 markets from Panta and upsert them into the DB.
/// Used by the bootstrap path in `list` and by the worker's sync job later.
pub async fn sync_first_pages(state: &AppState, pages: usize) -> Result<usize, AppError> {
    let client = state.db.get().await?;
    let mut cursor: Option<String> = None;
    let mut written = 0usize;

    for _ in 0..pages {
        let resp = state
            .panta
            .list_markets(&crate::panta::MarketFilter {
                cursor: cursor.clone(),
                limit: Some(50),
                ..Default::default()
            })
            .await?;

        for m in &resp.items {
            queries::markets::upsert_market(&client, m).await?;
            written += 1;
        }

        match resp.next_cursor {
            Some(next) => cursor = Some(next),
            None => break, // catalog exhausted
        }
    }
    Ok(written)
}

/// `GET /v1/markets` — browse/search/sort the local catalog cache.
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<MarketsQuery>,
) -> Result<Json<Page<MarketSummary>>, AppError> {
    let mut client = state.db.get().await?;

    // Bootstrap: a brand-new dev database has no markets until the worker
    // runs; pull the first pages once so the app is usable immediately.
    if queries::markets::count(&client).await? == 0 {
        drop(client); // don't hold the connection across the Panta call
        let written = sync_first_pages(&state, 3).await?;
        tracing::info!(written, "bootstrapped market catalog from panta");
        client = state.db.get().await?;
    }

    // Offset cursor: our own pagination scheme, opaque to the frontend.
    let cursor = q
        .cursor
        .as_deref()
        .map(str::parse::<usize>)
        .transpose()
        .map_err(|_| AppError::bad_request("INVALID_CURSOR", "cursor must be a number"))?;

    // Clamp the page size to something sane.
    let limit = q.limit.unwrap_or(24).clamp(1, 50) as usize;

    let (items, next_cursor) = queries::markets::list_markets(
        &client,
        q.q.as_deref(),
        q.category.as_deref(),
        q.sort.as_deref(),
        cursor,
        limit,
    )
    .await?;

    Ok(Json(Page {
        items,
        next_cursor: next_cursor.map(|n| n.to_string()),
    }))
}

/// Reject market ids that are not plain Panta ids before they are used in a
/// Panta URL path or a query (no `/`, `..`, `?`, spaces, …).
pub fn check_market_id(market_id: &str) -> Result<(), AppError> {
    if crate::panta::is_valid_market_id(market_id) {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "INVALID_MARKET_ID",
            "market id is malformed",
        ))
    }
}

/// Make sure a market row exists locally (orders and basket legs reference
/// it), fetching it from Panta once if we have never seen it.
pub async fn ensure_market(state: &AppState, market_id: &str) -> Result<(), AppError> {
    check_market_id(market_id)?;
    let client = state.db.get().await?;
    if queries::markets::get_market(&client, market_id)
        .await?
        .is_none()
    {
        drop(client); // don't hold the connection across the Panta call
        let row = state.panta.get_market(market_id).await?;
        let client = state.db.get().await?;
        queries::markets::upsert_market(&client, &row).await?;
    }
    Ok(())
}

/// `GET /v1/markets/{id}` — one market. Falls back to a live Panta fetch
/// (+ upsert) when we have never seen this market.
pub async fn detail(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> Result<Json<MarketDetail>, AppError> {
    check_market_id(&market_id)?;
    {
        let client = state.db.get().await?;
        if let Some(detail) = queries::markets::market_detail(&client, &market_id).await? {
            return Ok(Json(detail));
        }
    }

    // Unknown locally: ask Panta once, cache the answer, serve it.
    ensure_market(&state, &market_id).await?;
    let client = state.db.get().await?;
    queries::markets::market_detail(&client, &market_id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::internal("market vanished right after upsert"))
}

/// `GET /v1/markets/{id}/history?range=1d|7d|all` — our own price snapshots
/// (D10). Empty until snapshots exist: the UI shows "history since we
/// started tracking".
pub async fn history(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<MarketHistory>, AppError> {
    check_market_id(&market_id)?;
    let range = params.get("range").map(String::as_str).unwrap_or("all");
    let since = match range {
        "1d" => Some(Utc::now() - chrono::Duration::days(1)),
        "7d" => Some(Utc::now() - chrono::Duration::days(7)),
        _ => None, // "all"
    };

    let client = state.db.get().await?;
    let points = queries::markets::list_snapshots(&client, &market_id, since).await?;

    Ok(Json(MarketHistory {
        market_id,
        range: range.to_string(),
        points,
    }))
}

/// How long a market's trade tape is cached (protects our Panta read budget
/// from anonymous traffic; the tape is "recent trades", not tick data).
const TRADES_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

type TradesCache = HashMap<String, (std::time::Instant, Vec<MarketTrade>)>;
static TRADES_CACHE: std::sync::LazyLock<Mutex<TradesCache>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// `GET /v1/markets/{id}/trades` — the public trade tape (Panta data,
/// proxied through our backend so the API key never reaches the browser).
/// Cached per market for 30 s.
pub async fn trades(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> Result<Json<Vec<MarketTrade>>, AppError> {
    check_market_id(&market_id)?;
    if let Some((at, cached)) = TRADES_CACHE.lock().await.get(&market_id) {
        if at.elapsed() < TRADES_CACHE_TTL {
            return Ok(Json(cached.clone()));
        }
    }

    let rows = state.panta.market_trades(&market_id, 50).await?;
    let trades: Vec<MarketTrade> = rows
        .into_iter()
        .map(|t| {
            // Tape rows carry share amounts per side; a zero/absent amount
            // means the trade was on the other side.
            let nonzero = |v: &Option<crate::panta::StringOrNum>| {
                v.as_ref()
                    .map(|v| v.as_str())
                    .filter(|s| !s.trim_start_matches(['0', '.']).is_empty())
            };
            let (yes, no) = (nonzero(&t.yes_amount), nonzero(&t.no_amount));
            let side = match (&yes, &no) {
                (Some(_), None) => Some(crate::domain::Side::Yes),
                (None, Some(_)) => Some(crate::domain::Side::No),
                _ => None,
            };
            MarketTrade {
                signature: t.signature,
                side,
                // The tape has no USDC notional (only shares + fee), so we
                // do not invent one.
                amount_usdc: None,
                shares: yes.or(no),
                price: None, // Panta's tape has amounts, not per-trade prices
                ts: t
                    .block_time
                    .and_then(|s| chrono::Utc.timestamp_opt(s, 0).single()),
            }
        })
        .collect();

    TRADES_CACHE
        .lock()
        .await
        .insert(market_id, (std::time::Instant::now(), trades.clone()));
    Ok(Json(trades))
}
