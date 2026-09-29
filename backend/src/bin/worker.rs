//! Background-jobs binary. Run with `cargo run --bin worker`.
//! Jobs (market sync, price snapshots, order confirmation, …) are added in
//! later steps; for now this just proves the binary boots.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = backend::config::Config::from_env()?;
    tracing::info!(env = %config.app_env, "worker started (no jobs yet)");
    Ok(())
}
