//! `price_snapshot` — record price history (D10: our own snapshots, since
//! Panta has no history endpoint).
//!
//! Markets that appear in a PUBLISHED basket get a snapshot on every tick
//! (1 min — the brief calls for faster sampling of basket markets); every
//! other open market gets one every 5th tick (5 min effective).

use crate::queries;
use crate::state::AppState;

/// Which tick we're on (drives the slower cadence for non-basket markets).
static TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub async fn run_once(state: AppState) -> anyhow::Result<()> {
    let tick = TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let slow_tick = tick % 5 == 0;

    let client = state.db.get().await?;

    // Market ids that live in a published basket leg.
    let basket_market_ids: Vec<String> = client
        .query(
            "SELECT DISTINCT l.market_id FROM basket_legs l
             JOIN basket_templates t ON t.id = l.template_id
             WHERE t.status = 'published'",
            &[],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();

    let mut snapshots = 0usize;
    for (id, yes, no, volume) in queries::markets::open_market_prices(&client).await? {
        let in_basket = basket_market_ids.contains(&id);
        // Basket markets: every tick. Everything else: every 5th tick.
        if !in_basket && !slow_tick {
            continue;
        }
        // Skip rows without any price yet — nothing meaningful to record.
        if yes.is_none() && no.is_none() {
            continue;
        }
        queries::markets::insert_snapshot(&client, &id, yes, no, volume).await?;
        snapshots += 1;
    }

    tracing::debug!(snapshots, tick, "price_snapshot: recorded prices");
    Ok(())
}
