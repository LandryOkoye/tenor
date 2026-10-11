//! All SQL for the app, organized one module per table/area.
//!
//! Every function takes a `&tokio_postgres::Client` (or the pool) so route
//! handlers and worker jobs share the exact same queries. Results are plain
//! domain or lightweight row structs — no ORM magic.

pub mod baskets;
pub mod markets;
pub mod orders;
pub mod portfolio;
pub mod resolution;
pub mod users;
pub mod vault;
