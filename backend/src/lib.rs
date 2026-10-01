//! Backend library: shared modules used by the `api` and `worker` binaries.
//!
//! Layout (one folder per concern, in the order a request flows):
//! - `config`   – typed view of the environment variables (see .env.example)
//! - `error`    – the single error envelope every endpoint returns
//! - `domain`   – request/response types; also the source of the generated
//!   TypeScript bindings for the frontend (run `./scripts/typegen.sh`)
//! - `db`       – PostgreSQL pool, migration runner and typed queries
//! - `panta`    – typed client for the Panta API + a fixture-backed mock
//! - `solana_tx`– transaction assembly, simulation, validation, broadcast
//! - `basket`   – basket allocation math and quote/build orchestration
//! - `routes`   – axum handlers (only used by the `api` binary)
//! - `jobs`     – background jobs (only run by the `worker` binary)

pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod panta;
pub mod routes;
pub mod state;
