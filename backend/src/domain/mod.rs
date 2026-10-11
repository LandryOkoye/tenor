//! API contract types shared between the Rust backend and the TypeScript frontend.
//!
//! Single source of truth for `packages/api-types` (generated with ts-rs).
//! Run `cargo test -p domain export_bindings` to regenerate the TypeScript bindings.
//!
//! Wire conventions (see PROJECT_BRIEF.md §6.3):
//! - Money (`*_usdc`) is an integer **string** of USDC base units (6 decimals). Never floats.
//! - Prices are decimal strings in `0..=1` (e.g. `"0.62"` == 62¢ == 62% implied probability).
//! - Share counts are integer strings.
//! - Timestamps are RFC 3339 UTC strings.

pub mod admin;
pub mod auth;
pub mod basket;
pub mod common;
pub mod market;
pub mod order;
pub mod portfolio;
pub mod resolution;
pub mod vault;

pub use admin::*;
pub use auth::*;
pub use basket::*;
pub use common::*;
pub use market::*;
pub use order::*;
pub use portfolio::*;
pub use resolution::*;
pub use vault::*;
