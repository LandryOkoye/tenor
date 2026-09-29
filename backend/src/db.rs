//! PostgreSQL access layer.
//!
//! Design (deliberately simple, no ORM):
//! - Plain SQL migrations in `backend/migrations/*.sql`, embedded into the
//!   binary at compile time and applied in order at startup.
//! - A `deadpool-postgres` connection pool on top of the plain `tokio-postgres`
//!   driver; queries themselves live in the modules that use them.

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

/// Alias so the rest of the code talks about `db::DbPool`, not a vendor type.
pub type DbPool = Pool;

/// Every migration as `(name, sql)`, embedded at compile time.
///
/// To add a migration:
/// 1. create `backend/migrations/NNNN_description.sql`,
/// 2. add a row here. The runner records applied names in `schema_migrations`
///    so it never applies the same file twice.
pub const MIGRATIONS: &[(&str, &str)] = &[(
    "0001_initial",
    include_str!("../migrations/0001_initial.sql"),
)];

/// Parse a `postgres://user:pass@host:port/db` URL and build a connection pool.
///
/// The pool is lazy: connections are opened on first use, so a bad URL only
/// fails when something actually queries the database (startup keeps working,
/// which `/readyz` then reports honestly).
pub fn connect(database_url: &str) -> anyhow::Result<DbPool> {
    // tokio-postgres can parse the standard postgres:// URL directly.
    let pg_config: tokio_postgres::Config = database_url.parse()?;

    let manager = Manager::from_config(
        pg_config,
        NoTls, // local/managed Postgres over a private network does not need TLS
        ManagerConfig {
            // "Fast" = prefer reusing an idle connection over health-checking it.
            recycling_method: RecyclingMethod::Fast,
        },
    );

    let pool = Pool::builder(manager)
        .max_size(16)
        .build()
        .map_err(|e| anyhow::anyhow!("could not build db pool: {e}"))?;

    Ok(pool)
}

/// Apply all pending migrations. Idempotent — safe to call at every startup.
///
/// Each migration runs inside its own transaction: it either fully applies or
/// not at all, and the recorded name is only inserted after a successful commit.
pub async fn migrate(pool: &DbPool) -> anyhow::Result<()> {
    let mut client = pool.get().await?;

    // Bookkeeping table first (it cannot be part of migration 0001 because
    // the runner needs it before reading 0001's status).
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                 name       TEXT PRIMARY KEY,
                 applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
             )",
        )
        .await?;

    for (name, sql) in MIGRATIONS {
        // Loop items are &&str; take one reference off for readability.
        let name: &str = name;

        // Already applied? Skip. `query_opt` returns the row if it exists.
        let already_applied = client
            .query_opt("SELECT 1 FROM schema_migrations WHERE name = $1", &[&name])
            .await?
            .is_some();
        if already_applied {
            continue;
        }

        // Transaction: apply the SQL, then record the name — atomically.
        let tx = client.transaction().await?;
        tx.batch_execute(sql).await?;
        tx.execute("INSERT INTO schema_migrations (name) VALUES ($1)", &[&name])
            .await?;
        tx.commit().await?;

        tracing::info!(migration = name, "applied database migration");
    }

    Ok(())
}
