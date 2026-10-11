use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::domain::market::Side;
use crate::domain::order::TxSummary;

/// A tokenized basket (on-chain `basket_vault`, ADR 0002).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VaultBasketView {
    pub template_id: Uuid,
    pub slug: String,
    pub title: String,
    /// `pending` (create tx not confirmed) | `live` | `closed` | `settled`
    pub status: String,
    /// Basket token mint (SPL, 6 decimals).
    pub mint: String,
    pub vault: String,
    pub legs: Vec<VaultLegView>,
    /// Net asset value in USDC base units, read on chain (Tenor analysis).
    pub nav_usdc: Option<String>,
    /// Basket token supply (base units).
    pub supply: Option<String>,
    /// USDC per whole token, decimal string (e.g. "1.0234").
    pub nav_per_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VaultLegView {
    pub market_id: String,
    pub market_title: Option<String>,
    pub side: Side,
    pub weight_bps: i32,
}

/// `POST /v1/vault/baskets/{template_id}/deposit`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VaultDepositRequest {
    /// USDC base units, integer string (min 1 USDC).
    pub amount_usdc: String,
    /// Tolerance on minted tokens vs simulation (1–1000, default 150).
    pub slippage_bps: Option<i32>,
}

/// `POST /v1/vault/baskets/{template_id}/redeem`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VaultRedeemRequest {
    /// Basket tokens to burn, base units.
    pub tokens: String,
}

/// Build response for every vault transaction (create, deposit, redeem,
/// settle). Sign `transactions` and send them to `/v1/vault/txs/{tx_id}/submit`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VaultBuildResponse {
    pub tx_id: Uuid,
    pub transactions: Vec<String>,
    pub summary: TxSummary,
    /// Deposits: tokens the simulation minted (the tx enforces at least
    /// `min_tokens_out` = this minus slippage).
    pub expected_tokens: Option<String>,
    pub min_tokens_out: Option<String>,
}

/// `GET /v1/vault/txs/{tx_id}` and submit response.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VaultTxView {
    pub tx_id: Uuid,
    /// `create` | `deposit` | `redeem` | `settle` | `close`
    pub kind: String,
    /// `built` | `submitted` | `confirmed` | `failed`
    pub status: String,
    pub amount_usdc: Option<String>,
    pub tokens: Option<String>,
    pub tx_signature: Option<String>,
    pub error_code: Option<String>,
    pub created_at: DateTime<Utc>,
}
