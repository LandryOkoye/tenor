//! Background jobs (brief §11), run by the `worker` binary.
//!
//! One module per job; `run()` spawns them all. Every job is a simple loop:
//! do the work, log the outcome, sleep. Every job body is idempotent —
//! restarting the worker at any moment is always safe.
//!
//! | Job             | Cadence | Purpose                                    |
//! |-----------------|---------|--------------------------------------------|
//! | market_sync     | 2 min   | Refresh catalog; record status-change events |
//! | price_snapshot  | 1 min   | Price history rows (basket legs every tick) |
//! | basket_guard    | 5 min   | Close templates with non-tradable legs      |
//! | resolution_watch| 5 min   | Settle legs + purchases on resolved markets |
//! | tx_confirm      | 15 s    | Settle submitted txs (confirm/fail/expire)  |

pub mod basket_guard;
pub mod market_sync;
pub mod price_snapshot;
pub mod resolution_watch;
pub mod tx_confirm;

use std::time::Duration;

use crate::state::AppState;

/// Spawn every job on the tokio runtime. Runs forever; jobs never abort the
/// process — a failing job logs and retries on its next tick.
pub async fn run(state: AppState) {
    let sync = tokio::spawn(tick(
        state.clone(),
        2 * 60,
        market_sync::run_once,
        "market_sync",
    ));
    let snap = tokio::spawn(tick(
        state.clone(),
        60,
        price_snapshot::run_once,
        "price_snapshot",
    ));
    let guard = tokio::spawn(tick(
        state.clone(),
        5 * 60,
        basket_guard::run_once,
        "basket_guard",
    ));
    let watch = tokio::spawn(tick(
        state.clone(),
        5 * 60,
        resolution_watch::run_once,
        "resolution_watch",
    ));
    let confirm = tokio::spawn(tick(state, 15, tx_confirm::run_once, "tx_confirm"));

    // If any job loop dies (e.g. a panic inside a job), report it.
    for (name, handle) in [
        ("market_sync", sync),
        ("price_snapshot", snap),
        ("basket_guard", guard),
        ("resolution_watch", watch),
        ("tx_confirm", confirm),
    ] {
        if let Err(e) = handle.await {
            tracing::error!(job = name, error = ?e, "job loop terminated");
        }
    }
}

/// Generic job loop: `run_once` on start, then every `interval_secs`.
/// Errors are logged, never fatal — the next tick retries.
async fn tick<F, Fut>(state: AppState, interval_secs: u64, job: F, name: &'static str)
where
    F: Fn(AppState) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    loop {
        let started = std::time::Instant::now();
        match job(state.clone()).await {
            Ok(()) => tracing::debug!(job = name, elapsed = ?started.elapsed(), "tick done"),
            Err(e) => tracing::warn!(job = name, error = ?e, "tick failed; will retry next cycle"),
        }
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}
