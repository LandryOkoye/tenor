//! HTTP API binary. Run with `cargo run --bin api`.
//!
//! Startup order: config -> db pool -> migrations -> router (with CORS) -> serve.

use std::sync::Arc;

use axum::http::{header, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use backend::config::Config;
use backend::routes;
use backend::state::AppState;

/// Build the CORS policy from config.
///
/// - No `CORS_ORIGINS` set (local dev): allow everything so the Vite dev
///   server on :5173 just works.
/// - Origins set (production): only those exact origins may call the API.
fn cors_layer(config: &Config) -> CorsLayer {
    if config.cors_origins.is_empty() {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        let origins = config
            .cors_origins
            .iter()
            .filter_map(|o| o.parse::<HeaderValue>().ok())
            .collect::<Vec<_>>();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Structured logging; verbosity via RUST_LOG (defaults to "info").
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // 1. Read and validate the environment.
    let config = Config::from_env()?;

    // 2. Connect to PostgreSQL and apply any pending migrations.
    let pool = backend::db::connect(&config.database_url)?;
    backend::db::migrate(&pool).await?;

    // 3. Build the Panta client (mock in dev, real HTTPS when configured)
    //    and the Solana transaction context (RPC + fee wallet + allowlist).
    let panta = backend::panta::make_client(&config);
    let solana = std::sync::Arc::new(backend::solana_tx::SolanaCtx::from_config(&config)?);

    // 4. Assemble the router with shared state and CORS.
    let state = AppState {
        db: pool,
        config: Arc::new(config.clone()),
        panta,
        solana,
    };
    let app = routes::router(state).layer(cors_layer(&config));

    // 4. Serve.
    let listener = tokio::net::TcpListener::bind(&config.api_bind).await?;
    tracing::info!(bind = %config.api_bind, "api listening");
    axum::serve(listener, app).await?;
    Ok(())
}
