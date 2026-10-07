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
    client
        .execute(
            "INSERT INTO claims (user_id, market_id, tx_signature, amount_usdc, status)
             VALUES ($1, $2, $3, $4, 'confirmed')",
            &[&user_id, &market_id, &tx_signature, &amount_usdc],
        )
        .await?;
    Ok(())
}

/// Mark all leg orders on a market as claimed.
pub async fn mark_orders_claimed(
    client: &Client,
    market_id: &str,
) -> Result<u64, tokio_postgres::Error> {
    let n = client
        .execute(
            "UPDATE orders SET status = 'claimed', updated_at = now()
             WHERE market_id = $1 AND status = 'resolved_win'",
            &[&market_id],
        )
        .await?;
    Ok(n)
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
            kind: if r.get::<_, String>(0) == "claim" { "claim" } else { "buy" },
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
