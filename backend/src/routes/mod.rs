//! HTTP route handlers, one module per area of the API.
//!
//! `router()` assembles the full axum router. As later build steps add
//! endpoints (markets, orders, baskets, …) they register themselves here.

pub mod health;

use axum::Router;

use crate::state::AppState;

/// The full router: health checks now; `/v1/*` routes are added in later steps.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", axum::routing::get(health::healthz))
        .route("/readyz", axum::routing::get(health::readyz))
        .with_state(state)
}
