//! HTTP route handlers, one module per area of the API.
//!
//! `router()` assembles the full axum router under `/v1` plus health checks.

pub mod admin;
pub mod auth;
pub mod baskets;
pub mod claims;
pub mod health;
pub mod markets;
pub mod orders;
pub mod portfolio;
pub mod resolution;
pub mod vault;

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

    // Baskets: public browsing, member trading (brief §9).
    let basket_routes = Router::new()
        .route("/v1/baskets", get(baskets::list))
        .route("/v1/baskets/{slug}", get(baskets::detail))
        .route("/v1/baskets/{id}/quote", post(baskets::quote))
        .route("/v1/baskets/quotes/{quote_id}/build", post(baskets::build))
        .route("/v1/baskets/purchases/{id}/submit", post(baskets::submit))
        .route("/v1/baskets/purchases/{id}", get(baskets::purchase_status));

    // Portfolio + claims (session).
    let portfolio_routes = Router::new()
        .route("/v1/portfolio", get(portfolio::portfolio))
        .route("/v1/portfolio/activity", get(portfolio::activity))
        .route("/v1/claims/build", post(claims::build))
        .route("/v1/claims/submit", post(claims::submit));

    // Resolution Center (public reads; Tenor analysis clearly labelled).
    let resolution_routes = Router::new()
        .route("/v1/resolution/markets", get(resolution::markets))
        .route("/v1/resolution/markets/{id}", get(resolution::detail))
        .route(
            "/v1/resolution/creators/{creator}",
            get(resolution::creator),
        );

    // Admin curation (AdminUser extractor: JWT + wallet allowlist).
    let admin_routes = Router::new()
        .route("/v1/admin/panta-markets", get(admin::search_markets))
        .route("/v1/admin/baskets", post(admin::create))
        .route(
            "/v1/admin/baskets/{id}",
            axum::routing::patch(admin::update),
        )
        .route("/v1/admin/baskets/{id}/publish", post(admin::publish))
        .route("/v1/admin/baskets/{id}/close", post(admin::close))
        .route("/v1/admin/baskets/{id}/tokenize", post(vault::tokenize))
        .route("/v1/admin/baskets/{id}/vault-close", post(vault::close))
        .route("/v1/admin/baskets/{id}/vault-settle", post(vault::settle));

    // Tokenized baskets (on-chain basket_vault, ADR 0002).
    let vault_routes = Router::new()
        .route("/v1/vault/baskets/{slug}", get(vault::basket))
        .route("/v1/vault/baskets/{id}/deposit", post(vault::deposit))
        .route("/v1/vault/baskets/{id}/redeem", post(vault::redeem))
        .route("/v1/vault/txs/{id}/submit", post(vault::submit))
        .route("/v1/vault/txs/{id}", get(vault::tx_status));

    health_routes
        .merge(auth_routes)
        .merge(market_routes)
        .merge(order_routes)
        .merge(basket_routes)
        .merge(portfolio_routes)
        .merge(resolution_routes)
        .merge(admin_routes)
        .merge(vault_routes)
        .with_state(state)
}
