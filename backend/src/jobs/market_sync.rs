//! `market_sync` — keep the local catalog in sync with Panta (brief §11).
//!
//! Pages through the Panta catalog, upserts every row, and records a
//! `market_events` row whenever a market's phase/status changes (or when we
//! first see a market). For fresh rows without prices, fetches the detail
//! endpoint once so quotes and portfolio views always have numbers.

use crate::queries;
use crate::state::AppState;

/// How many catalog pages (of 50) we walk per tick.
const SYNC_PAGES: usize = 10;
/// How many price-less markets get a detail fetch per tick.
const DETAIL_FETCH_LIMIT: i64 = 50;

pub async fn run_once(state: AppState) -> anyhow::Result<()> {
    let client = state.db.get().await?;

    // 1. Walk the catalog, upserting each row and diffing changes.
    let mut cursor: Option<String> = None;
    let mut written = 0usize;
    let mut changed = 0usize;
    for _ in 0..SYNC_PAGES {
        let page = state
            .panta
            .list_markets(&crate::panta::MarketFilter {
                cursor: cursor.clone(),
                limit: Some(50),
                ..Default::default()
            })
            .await?;

        for m in &page.items {
            // Change detection BEFORE the upsert (brief §8: events feed the
            // Resolution Center). First sighting counts as "created".
            let before = queries::markets::status_tuple(&client, &m.market_id).await?;
            queries::markets::upsert_market(&client, m).await?;
            written += 1;

            let event = match &before {
                None => Some(("created", serde_json::json!({ "phase": m.phase }))),
                Some((phase, status, resolved)) => {
                    if *phase != m.phase || *status != m.status || *resolved != m.resolved {
                        Some((
                            "status_change",
                            serde_json::json!({
                                "from": { "phase": phase, "status": status, "resolved": resolved },
                                "to":   { "phase": m.phase, "status": m.status, "resolved": m.resolved },
                            }),
                        ))
                    } else {
                        None
                    }
                }
            };
            if let Some((kind, payload)) = event {
                queries::markets::insert_event(&client, &m.market_id, kind, &payload).await?;
                changed += 1;
            }
        }

        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }

    // 2. Fill in prices for fresh rows (list rows often have NULL prices).
    let mut priced = 0usize;
    for id in queries::markets::markets_missing_prices(&client, DETAIL_FETCH_LIMIT).await? {
        match state.panta.get_market(&id).await {
            Ok(detail) => {
                queries::markets::upsert_market(&client, &detail).await?;
                priced += 1;
            }
            Err(e) => tracing::warn!(market = %id, error = %e, "detail fetch failed"),
        }
    }

    tracing::info!(written, changed, priced, "market_sync: catalog refreshed");
    Ok(())
}
