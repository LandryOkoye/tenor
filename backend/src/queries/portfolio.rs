//! Portfolio queries (brief §7.6): our order history joined with market
//! prices — cost basis, per-leg views, activity feed.

use chrono::{DateTime, Utc};
use tokio_postgres::Client;
use uuid::Uuid;

/// One of OUR orders enriched with current market numbers for display.
pub struct PortfolioOrder {
    pub id: Uuid,
    pub basket_purchase_id: Option<Uuid>,
    pub market_id: String,
    pub market_title: Option<String>,
    pub side: String,
    pub amount_usdc: i64,
    pub expected_shares: Option<i64>,
    pub status: String,
    pub tx_signature: Option<String>,
    pub created_at: DateTime<Utc>,
    // live market fields:
    pub yes_price: Option<String>,
    pub no_price: Option<String>,
    pub phase: Option<String>,
    pub resolved: bool,
    pub outcome: Option<String>,
}

/// Every order of a user (any status; the caller filters).
pub async fn orders_for_user(
    client: &Client,
    user_id: Uuid,
) -> Result<Vec<PortfolioOrder>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT o.id, o.basket_purchase_id, o.market_id, m.title, o.side, o.amount_usdc,
                    o.expected_shares, o.status, o.tx_signature, o.created_at,
                    m.yes_price, m.no_price, m.phase, m.resolved, m.outcome
             FROM orders o LEFT JOIN markets m ON m.id = o.market_id
             WHERE o.user_id = $1
             ORDER BY o.created_at DESC",
            &[&user_id],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| PortfolioOrder {
            id: r.get(0),
            basket_purchase_id: r.get(1),
            market_id: r.get(2),
            market_title: r.get(3),
            side: r.get(4),
            amount_usdc: r.get(5),
            expected_shares: r.get(6),
            status: r.get(7),
            tx_signature: r.get(8),
            created_at: r.get(9),
            yes_price: r.get(10),
            no_price: r.get(11),
            phase: r.get(12),
            resolved: r.get(13),
            outcome: r.get(14),
        })
        .collect())
}

/// Record a completed win claim (after the signed claim tx confirms).
pub async fn insert_claim(
    client: &Client,
    user_id: Uuid,
    market_id: &str,
    tx_signature: &str,
    amount_usdc: Option<i64>,
) -> Result<(), tokio_postgres::Error> {
    // Idempotent: the API and the tx_confirm job may both settle a claim.
    client
        .execute(
            "INSERT INTO claims (user_id, market_id, tx_signature, amount_usdc, status)
             SELECT $1, $2, $3, $4, 'confirmed'
             WHERE NOT EXISTS (SELECT 1 FROM claims
                               WHERE user_id = $1 AND market_id = $2 AND tx_signature = $3)",
            &[&user_id, &market_id, &tx_signature, &amount_usdc],
        )
        .await?;
    Ok(())
}

/// Mark one user's winning orders on a market as claimed.
pub async fn mark_orders_claimed(
    client: &Client,
    user_id: Uuid,
    market_id: &str,
) -> Result<u64, tokio_postgres::Error> {
    let n = client
        .execute(
            "UPDATE orders SET status = 'claimed', updated_at = now()
             WHERE user_id = $1 AND market_id = $2 AND status = 'resolved_win'",
            &[&user_id, &market_id],
        )
        .await?;
    Ok(n)
}

// --- Claim batches (one per claim transaction we build) ----------------------

pub async fn insert_claim_batch(
    client: &Client,
    user_id: Uuid,
    market_ids: &[String],
    built_message: &[u8],
    last_valid_block_height: Option<i64>,
) -> Result<Uuid, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO claim_batches (user_id, market_ids, built_message,
                                        last_valid_block_height)
             VALUES ($1, $2, $3, $4) RETURNING id",
            &[
                &user_id,
                &market_ids,
                &built_message,
                &last_valid_block_height,
            ],
        )
        .await?;
    Ok(row.get(0))
}

pub struct ClaimBatchRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub market_ids: Vec<String>,
    pub built_message: Vec<u8>,
    pub last_valid_block_height: Option<i64>,
    pub tx_signature: Option<String>,
    pub status: String,
}

pub async fn get_claim_batch(
    client: &Client,
    id: Uuid,
) -> Result<Option<ClaimBatchRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, user_id, market_ids, built_message, last_valid_block_height,
                    tx_signature, status
             FROM claim_batches WHERE id = $1",
            &[&id],
        )
        .await?;
    Ok(row.map(|r| ClaimBatchRow {
        id: r.get(0),
        user_id: r.get(1),
        market_ids: r.get(2),
        built_message: r.get(3),
        last_valid_block_height: r.get(4),
        tx_signature: r.get(5),
        status: r.get(6),
    }))
}

/// Move a claim batch `from` -> `to` (no-op otherwise). Returns true if moved.
pub async fn transition_claim_batch(
    client: &Client,
    id: Uuid,
    from: &str,
    to: &str,
    signature: Option<&str>,
    error_code: Option<&str>,
) -> Result<bool, tokio_postgres::Error> {
    let n = client
        .execute(
            "UPDATE claim_batches
             SET status = $3, tx_signature = COALESCE($4, tx_signature),
                 error_code = $5, updated_at = now()
             WHERE id = $1 AND status = $2",
            &[&id, &from, &to, &signature, &error_code],
        )
        .await?;
    Ok(n == 1)
}

/// Activity feed pieces from our tables (brief: orders + claims merged).
pub struct ActivityRow {
    pub kind: &'static str, // "buy" | "claim"
    pub market_id: String,
    pub market_title: Option<String>,
    pub side: Option<String>,
    pub amount_usdc: i64,
    pub tx_signature: Option<String>,
    pub created_at: DateTime<Utc>,
    pub basket_purchase_id: Option<Uuid>,
}

/// Recent activity for a wallet: orders and claims, newest first, capped.
pub async fn activity(
    client: &Client,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<ActivityRow>, tokio_postgres::Error> {
    let rows = client
        .query(
            // UNION of filled orders and confirmed claims, newest first.
            "SELECT * FROM (
                SELECT 'buy' AS kind, o.market_id, m.title AS market_title, o.side,
                       o.amount_usdc, o.tx_signature, o.created_at, o.basket_purchase_id
                FROM orders o LEFT JOIN markets m ON m.id = o.market_id
                WHERE o.user_id = $1 AND o.status IN
                      ('filled', 'resolved_win', 'resolved_loss', 'claimed')
             UNION ALL
                SELECT 'claim' AS kind, c.market_id, m.title AS market_title,
                       NULL AS side, COALESCE(c.amount_usdc, 0), c.tx_signature,
                       c.created_at, NULL AS basket_purchase_id
                FROM claims c LEFT JOIN markets m ON m.id = c.market_id
                WHERE c.user_id = $1 AND c.status = 'confirmed'
             ) a
             ORDER BY created_at DESC
             LIMIT $2",
            &[&user_id, &limit],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| ActivityRow {
            kind: if r.get::<_, String>(0) == "claim" {
                "claim"
            } else {
                "buy"
            },
            market_id: r.get(1),
            market_title: r.get(2),
            side: r.get(3),
            amount_usdc: r.get(4),
            tx_signature: r.get(5),
            created_at: r.get(6),
            basket_purchase_id: r.get(7),
        })
        .collect())
}
