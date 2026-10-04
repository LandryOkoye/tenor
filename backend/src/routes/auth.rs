//! `POST /v1/auth/nonce` and `POST /v1/auth/verify` (SIWS login).

use axum::{extract::State, Json};
use chrono::{DateTime, Utc};

use crate::auth;
use crate::domain::{NonceRequest, NonceResponse, VerifyRequest, VerifyResponse};
use crate::error::AppError;
use crate::queries;
use crate::state::AppState;

/// `POST /v1/auth/nonce` — start a wallet login.
/// Returns the exact message the wallet must sign (contains the one-time nonce).
pub async fn nonce(
    State(state): State<AppState>,
    Json(req): Json<NonceRequest>,
) -> Result<Json<NonceResponse>, AppError> {
    // Basic sanity: a Solana pubkey is base58; reject obviously bad input.
    if bs58::decode(&req.wallet).into_vec().is_err() {
        return Err(AppError::bad_request("INVALID_WALLET", "wallet is not valid base58"));
    }

    let nonce = auth::new_nonce();
    let message = auth::sign_in_message(&req.wallet, &nonce);
    let expires_at = auth::nonce_expiry();

    let client = state.db.get().await?;
    queries::users::insert_nonce(&client, &nonce, &req.wallet, &message, expires_at).await?;

    Ok(Json(NonceResponse {
        nonce,
        message,
        expires_at,
    }))
}

/// `POST /v1/auth/verify` — finish a wallet login with a signature.
pub async fn verify(
    State(state): State<AppState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, AppError> {
    let client = state.db.get().await?;

    // 1. Find an unused, unexpired nonce for this wallet (we can't trust the
    //    client to send it back — the signature must be over OUR stored message).
    let Some((nonce, message)) = queries::users::find_live_nonce(&client, &req.wallet).await?
    else {
        return Err(AppError::bad_request(
            "NONCE_NOT_FOUND",
            "no active login request for this wallet; call /v1/auth/nonce first",
        ));
    };

    // 2. Cryptographic check: signature over the stored message by this wallet.
    auth::verify_wallet_signature(&req.wallet, &message, &req.signature)?;

    // 3. Consume the nonce exactly once (prevents replay).
    if !queries::users::consume_nonce(&client, &nonce).await? {
        return Err(AppError::bad_request("NONCE_ALREADY_USED", "login nonce already used"));
    }

    // 4. Upsert the user and mint the session token.
    let user_id = queries::users::upsert_user(&client, &req.wallet).await?;
    let (token, expires_at) = auth::mint_token(&state.config.jwt_secret, user_id, &req.wallet)?;

    tracing::info!(wallet = %req.wallet, "wallet login verified");
    Ok(Json(VerifyResponse {
        token,
        expires_at: DateTime::<Utc>::from(expires_at),
    }))
}
