//! `tx_confirm` — finish every broadcast transaction (brief §11).
//!
//! Submit endpoints record the signature BEFORE broadcasting and wait only
//! briefly. Anything still `submitted` lands here: confirmed signatures are
//! settled (Panta submit + attribution, records -> filled/confirmed), failed
//! ones are marked failed, and signatures never seen after their blockhash
//! expired are marked `TX_EXPIRED`. Idempotent: settlement only moves rows
//! that are still `submitted`.

use crate::queries;
use crate::settlement;
use crate::state::AppState;

/// How many pending signatures one tick looks at.
const BATCH: i64 = 100;

pub async fn run_once(state: AppState) -> anyhow::Result<()> {
    let pending = {
        let client = state.db.get().await?;
        queries::orders::pending_transactions(&client, BATCH).await?
    };
    if pending.is_empty() {
        return Ok(());
    }

    let height = state.solana.block_height().await?;
    let mut settled = 0usize;
    for tx in &pending {
        match settlement::reconcile(&state, tx, height).await {
            Ok(true) => settled += 1,
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(signature = %tx.signature, error = ?e, "tx_confirm: reconcile failed")
            }
        }
    }
    tracing::info!(pending = pending.len(), settled, "tx_confirm: tick");
    Ok(())
}
