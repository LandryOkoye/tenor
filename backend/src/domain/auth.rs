use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// `POST /v1/auth/nonce`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NonceRequest {
    pub wallet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NonceResponse {
    pub nonce: String,
    /// Exact message the wallet must sign (Sign-In-With-Solana style).
    pub message: String,
    pub expires_at: DateTime<Utc>,
}

/// `POST /v1/auth/verify`
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VerifyRequest {
    pub wallet: String,
    /// Base58 signature of `message` by the wallet keypair.
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VerifyResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}
