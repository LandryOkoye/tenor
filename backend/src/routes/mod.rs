//! HTTP route handlers, one module per area of the API.
//!
//! `router()` assembles the full axum router under `/v1` plus health checks.

pub mod auth;
pub mod health;
pub mod markets;
pub mod orders;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

/// The full router.
pub fn router(state: AppState) -> Router {
    // Health checks (no /v1 prefix).
    let health_routes = Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz));

    // Wallet login (SIWS).
    let auth_routes = Router::new()
        .route("/v1/auth/nonce", post(auth::nonce))
        .route("/v1/auth/verify", post(auth::verify));

    // Market browsing (public reads).
    let market_routes = Router::new()
        .route("/v1/categories", get(markets::categories))
        .route("/v1/markets", get(markets::list))
        .route("/v1/markets/{id}", get(markets::detail))
        .route("/v1/markets/{id}/history", get(markets::history))
        .route("/v1/markets/{id}/trades", get(markets::trades));

    // Single-market trading (session required via AuthUser extractor).
    let order_routes = Router::new()
        .route("/v1/orders/quote", post(orders::quote))
        .route("/v1/orders/{id}/build", post(orders::build))
        .route("/v1/orders/{id}/submit", post(orders::submit))
        .route("/v1/orders/{id}", get(orders::status));

    health_routes
        .merge(auth_routes)
        .merge(market_routes)
        .merge(order_routes)
        .with_state(state)
}
