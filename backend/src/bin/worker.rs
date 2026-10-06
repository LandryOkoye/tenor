//! Background-jobs binary. Run with `cargo run --bin worker`.
//!
//! Owns the recurring jobs: market sync, price snapshots, basket guard,
//! resolution watch (see `src/jobs/`). Runs until Ctrl-C.

use std::sync::Arc;

use backend::config::Config;
use backend::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_env()?;

    // Same setup as the API: shared schema, same Panta client.
    let pool = backend::db::connect(&config.database_url)?;
    backend::db::migrate(&pool).await?;
    let panta = backend::panta::make_client(&config);
    let solana = Arc::new(backend::solana_tx::SolanaCtx::from_config(&config)?);

    let state = AppState {
        db: pool,
        config: Arc::new(config.clone()),
        panta,
        solana,
    };

    tracing::info!(env = %config.app_env, "worker started");

    backend::jobs::run(state).await;
    Ok(())
}
