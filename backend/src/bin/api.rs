//! HTTP API binary. Run with `cargo run --bin api`.
//!
//! Wires together: config -> db pool + migrations -> router -> serve.

use axum::{routing::get, Json, Router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // JSON/structured logging; verbosity controlled by RUST_LOG.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = backend::config::Config::from_env()?;

    let app = Router::new().route(
        "/healthz",
        get(|| async {
            Json(backend::domain::Health {
                status: "ok".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            })
        }),
    );

    let listener = tokio::net::TcpListener::bind(&config.api_bind).await?;
    tracing::info!(bind = %config.api_bind, "api listening");
    axum::serve(listener, app).await?;
    Ok(())
}
