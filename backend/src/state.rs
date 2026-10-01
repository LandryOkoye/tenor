//! Shared application state handed to every HTTP handler.
//!
//! Cloned cheaply into every request (the pool handle and `Arc`s are just
//! reference-counted pointers — no connection is copied).

use std::sync::Arc;

use crate::config::Config;
use crate::db::DbPool;
use crate::panta::PantaApi;

#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub db: DbPool,
    /// Parsed environment configuration (read once at startup).
    pub config: Arc<Config>,
    /// Panta client: real HTTPS client, or the in-memory mock in dev mode.
    pub panta: Arc<dyn PantaApi>,
}
