//! User and auth-nonce queries.

use chrono::{DateTime, Utc};
use tokio_postgres::Client;
use uuid::Uuid;

/// Insert the user if new, refresh last_seen_at if returning. Returns user id.
pub async fn upsert_user(client: &Client, wallet: &str) -> Result<Uuid, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO users (wallet_pubkey)
             VALUES ($1)
             ON CONFLICT (wallet_pubkey) DO UPDATE SET last_seen_at = now()
             RETURNING id",
            &[&wallet],
        )
        .await?;
    Ok(row.get(0))
}

/// Create a sign-in nonce (SIWS): one-time secret embedded in the message the
/// wallet must sign. Expires after 5 minutes, single use.
pub async fn insert_nonce(
    client: &Client,
    nonce: &str,
    wallet: &str,
    message: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO auth_nonces (nonce, wallet, message, expires_at)
             VALUES ($1, $2, $3, $4)",
            &[&nonce, &wallet, &message, &expires_at],
        )
        .await?;
    Ok(())
}

/// Look up an unexpired, unused nonce for `wallet`. Does NOT mark it used —
/// the caller does that atomically after the signature verifies.
pub async fn find_live_nonce(
    client: &Client,
    wallet: &str,
) -> Result<Option<(String, String)>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT nonce, message FROM auth_nonces
             WHERE wallet = $1 AND used_at IS NULL AND expires_at > now()
             ORDER BY expires_at DESC LIMIT 1",
            &[&wallet],
        )
        .await?;
    Ok(row.map(|r| (r.get::<_, String>(0), r.get::<_, String>(1))))
}

/// Atomically consume a nonce: only succeeds if it was still unused.
/// Returns true when the nonce is now consumed by this call.
pub async fn consume_nonce(client: &Client, nonce: &str) -> Result<bool, tokio_postgres::Error> {
    let n = client
        .execute(
            "UPDATE auth_nonces SET used_at = now()
             WHERE nonce = $1 AND used_at IS NULL AND expires_at > now()",
            &[&nonce],
        )
        .await?;
    Ok(n == 1)
}
