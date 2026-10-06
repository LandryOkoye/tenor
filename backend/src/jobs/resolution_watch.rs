//! `resolution_watch` — detect resolutions and settle everything downstream
//! (brief §7.4, §11):
//! - leg orders on a resolved market become `resolved_win` / `resolved_loss`
//! - basket purchases move to `active` / `partially_resolved` / `resolved`
//! - a `market_events` row marks the resolution itself (Phase 2 timeline)
//!
//! Resolution detection happens in market_sync (phase change to `resolved`);
//! this job settles our bookkeeping for already-resolved markets.

use crate::queries;
use crate::state::AppState;

pub async fn run_once(state: AppState) -> anyhow::Result<()> {
    let client = state.db.get().await?;

    for (market_id, outcome) in queries::markets::resolved_markets(&client).await? {
        // Record the resolution event once (idempotent insert guarded below).
        let already_recorded = client
            .query_opt(
                "SELECT 1 FROM market_events
                 WHERE market_id = $1 AND event_type = 'resolved' LIMIT 1",
                &[&market_id],
            )
            .await?
            .is_some();
        if !already_recorded {
            queries::markets::insert_event(
                &client,
                &market_id,
                "resolved",
                &serde_json::json!({ "outcome": outcome }),
            )
            .await?;
            tracing::info!(market = %market_id, outcome = ?outcome, "resolution recorded");
        }

        // Settle legs + purchases (idempotent: only unsettled rows match).
        let legs =
            queries::baskets::settle_resolutions(&client, &market_id, outcome.as_deref()).await?;
        if legs > 0 {
            tracing::info!(market = %market_id, legs, "resolution settled leg orders");
        }
    }
    Ok(())
}
