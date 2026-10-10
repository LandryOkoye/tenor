//! Sign-In-With-Solana (SIWS) authentication (brief §9).
//!
//! Flow:
//! 1. Frontend asks `POST /v1/auth/nonce` with the wallet address.
//! 2. We create a one-time secret (nonce) and return a message that contains
//!    it. The wallet signs that exact message (no transaction, no fees).
//! 3. Frontend posts the signature to `POST /v1/auth/verify`.
//! 4. We verify the ed25519 signature against the wallet's public key,
//!    consume the nonce, upsert the user, and issue a short-lived JWT.
//! 5. Every user-scoped endpoint requires `Authorization: Bearer <jwt>` —
//!    checked by the `AuthUser` extractor below.
//!
//! We never see or store private keys; we only verify signatures.

use axum::{extract::FromRequestParts, http::request::Parts};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

/// How long a sign-in nonce stays valid.
const NONCE_TTL_SECS: i64 = 5 * 60;
/// How long a session token stays valid.
const SESSION_TTL_HOURS: i64 = 24;

/// The message a wallet must sign. Plain, honest, and nonce-bound so it can
/// never be replayed from another session.
pub fn sign_in_message(wallet: &str, nonce: &str) -> String {
    format!(
        "APP wants you to sign in with your Solana account:\n{wallet}\n\n\
         This request will not trigger any transaction or cost any fees.\n\n\
         Nonce: {nonce}"
    )
}

/// JWT claims — the only thing we need inside a session token.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionClaims {
    /// User id (uuid string).
    pub sub: String,
    /// Wallet pubkey (base58) — the identity for trading.
    pub wallet: String,
    /// Expiry (unix seconds; standard JWT field name).
    pub exp: i64,
}

/// Issue a session token for a user.
pub fn mint_token(
    secret: &str,
    user_id: Uuid,
    wallet: &str,
) -> Result<(String, chrono::DateTime<Utc>), AppError> {
    let expires_at = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
    let claims = SessionClaims {
        sub: user_id.to_string(),
        wallet: wallet.to_string(),
        exp: expires_at.timestamp(),
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::internal(format!("could not mint session token: {e}")))?;
    Ok((token, expires_at))
}

/// Verify an ed25519 signature produced by the wallet over the SIWS message.
/// `wallet` is base58; `signature` is base58 (the standard Solana encoding).
pub fn verify_wallet_signature(
    wallet: &str,
    message: &str,
    signature: &str,
) -> Result<(), AppError> {
    use ed25519_dalek::Verifier;

    // Decode the three base58 pieces.
    let pubkey_bytes = bs58::decode(wallet)
        .into_vec()
        .map_err(|_| AppError::bad_request("INVALID_WALLET", "wallet is not valid base58"))?;
    let sig_bytes = bs58::decode(signature)
        .into_vec()
        .map_err(|_| AppError::bad_request("INVALID_SIGNATURE", "signature is not valid base58"))?;

    if pubkey_bytes.len() != 32 {
        return Err(AppError::bad_request(
            "INVALID_WALLET",
            "wallet must be a 32-byte pubkey",
        ));
    }
    if sig_bytes.len() != 64 {
        return Err(AppError::bad_request(
            "INVALID_SIGNATURE",
            "signature must be 64 bytes",
        ));
    }

    // ed25519 verify: the only cryptographic trust decision in the app.
    let key_bytes: [u8; 32] = pubkey_bytes.try_into().expect("length checked above");
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&key_bytes).map_err(|_| {
        AppError::bad_request("INVALID_WALLET", "wallet bytes are not a valid ed25519 key")
    })?;
    let sig =
        ed25519_dalek::Signature::from_bytes(&sig_bytes.try_into().expect("length checked above"));
    verifying_key
        .verify(message.as_bytes(), &sig)
        .map_err(|_| AppError::unauthorized("signature does not match this wallet"))
}

/// Extractor: put `AuthUser` in a handler signature and axum runs the check.
///
/// Reads `Authorization: Bearer <jwt>`, verifies it, and hands the handler
/// the user id + wallet. No DB round-trip — the token is the proof.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub wallet: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 1. Pull the bearer token out of the headers.
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| AppError::unauthorized("missing Authorization: Bearer token"))?;

        // 2. Verify signature + expiry against our secret.
        let data = decode::<SessionClaims>(
            header,
            &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::unauthorized("session token is invalid or expired"))?;

        // 3. Hand the identity to the handler.
        let user_id = Uuid::parse_str(&data.claims.sub)
            .map_err(|_| AppError::unauthorized("session token has an invalid subject"))?;
        Ok(AuthUser {
            user_id,
            wallet: data.claims.wallet,
        })
    }
}

/// Generate the nonce value (opaque random string).
pub fn new_nonce() -> String {
    Uuid::new_v4().to_string()
}

/// Expiry for a freshly created nonce.
pub fn nonce_expiry() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::seconds(NONCE_TTL_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;

    /// Signature round-trip: a real ed25519 signature over the SIWS message
    /// verifies; a tampered message does not.
    #[test]
    fn signature_verification_round_trip() {
        // Deterministic key from a fixed seed — no RNG dependency needed.
        let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let wallet = bs58::encode(key.verifying_key().as_bytes()).into_string();
        let message = sign_in_message(&wallet, "nonce-123");

        // Sign exactly the message bytes.
        let sig = key.sign(message.as_bytes());
        let sig_b58 = bs58::encode(sig.to_bytes()).into_string();
        assert!(verify_wallet_signature(&wallet, &message, &sig_b58).is_ok());

        // Wrong message must fail.
        let other = sign_in_message(&wallet, "nonce-456");
        assert!(verify_wallet_signature(&wallet, &other, &sig_b58).is_err());

        // Garbage inputs must fail cleanly (not panic).
        assert!(verify_wallet_signature("not-base58!!", &message, &sig_b58).is_err());
        assert!(verify_wallet_signature(&wallet, &message, "short").is_err());
    }

    #[test]
    fn token_round_trip() {
        let user = Uuid::new_v4();
        let wallet = "11TestWa11et1111111111111111111111111111111";
        let (token, exp) = mint_token("secret", user, wallet).unwrap();
        assert!(exp > Utc::now());

        // Decode manually and confirm claims survive.
        let data = decode::<SessionClaims>(
            &token,
            &DecodingKey::from_secret("secret".as_bytes()),
            &Validation::default(),
        )
        .unwrap();
        assert_eq!(data.claims.sub, user.to_string());
        assert_eq!(data.claims.wallet, wallet);

        // Wrong secret must reject.
        assert!(decode::<SessionClaims>(
            &token,
            &DecodingKey::from_secret("other".as_bytes()),
            &Validation::default(),
        )
        .is_err());
    }
}
