//! Background-jobs binary. Run with `cargo run --bin worker`.
//!
//! Owns the recurring jobs (market sync, price snapshots, transaction
//! confirmation, …). The jobs themselves are added in later build steps; for
//! now the worker connects to PostgreSQL, applies migrations, and idles until
//! Ctrl-C — which proves the database path works end to end.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = backend::config::Config::from_env()?;

    // Same database setup as the API: both binaries share the schema.
    let pool = backend::db::connect(&config.database_url)?;
    backend::db::migrate(&pool).await?;

    tracing::info!(env = %config.app_env, "worker started (no jobs configured yet)");

    // Idle until Ctrl-C so the process behaves like a daemon from day one.
    tokio::signal::ctrl_c().await?;
    tracing::info!("worker shutting down");
    Ok(())
}
