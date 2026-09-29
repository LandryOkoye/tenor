//! Liveness and readiness probes.
//!
//! - `/healthz` answers "is the process alive?" — never touches dependencies,
//!   so an overloaded database does not get the process killed.
//! - `/readyz` answers "can we serve traffic?" — checks PostgreSQL with a
//!   trivial query. RPC and Panta checks are wired in when those clients are
//!   built (later steps); until then they report `null` (not checked).

use axum::{extract::State, http::StatusCode, Json};

use crate::domain::Ready;
use crate::state::AppState;

/// `GET /healthz` — liveness probe.
pub async fn healthz() -> Json<crate::domain::Health> {
    Json(crate::domain::Health {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// `GET /readyz` — readiness probe. Returns 503 when a dependency is down.
pub async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Ready>) {
    // Ask the pool for a connection and run a trivial query: this proves both
    // that the pool can hand out connections and that PostgreSQL answers.
    let db_ok = match state.db.get().await {
        Ok(client) => client.query_one("SELECT 1", &[]).await.is_ok(),
        Err(e) => {
            tracing::error!(error = %e, "readyz: could not get db connection");
            false
        }
    };

    let ready = Ready {
        db: db_ok,
        rpc: None,   // Solana RPC check added in the solana-tx build step
        panta: None, // Panta check added in the panta-client build step
    };

    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(ready))
}
