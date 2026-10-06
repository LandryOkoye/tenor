//! Order queries: the single-market buy lifecycle (brief §9).
//!
//! Statuses stored in `orders.status`:
//! `quoted -> built -> submitted -> confirmed -> filled | failed`

use chrono::{DateTime, Utc};
use tokio_postgres::Client;
use uuid::Uuid;

/// Create an order row after Panta accepts our quote.
#[allow(clippy::too_many_arguments)]
pub async fn insert_order(
    client: &Client,
    user_id: Uuid,
    market_id: &str,
    side: &str,                   // "yes" | "no"
    amount_usdc: i64,             // base units
    expected_shares: Option<i64>, // base units
    panta_quote_id: &str,
) -> Result<Uuid, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO orders (user_id, market_id, side, amount_usdc,
                                 expected_shares, panta_quote_id, status)
             VALUES ($1, $2, $3, $4, $5, $6, 'quoted')
             RETURNING id",
            &[
                &user_id,
                &market_id,
                &side,
                &amount_usdc,
                &expected_shares,
                &panta_quote_id,
            ],
        )
        .await?;
    Ok(row.get(0))
}

/// Everything we need to know about an order (with market title for display).
pub struct OrderRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub market_id: String,
    pub market_title: Option<String>,
    pub side: String,
    pub amount_usdc: i64,
    pub expected_shares: Option<i64>,
    pub panta_quote_id: Option<String>,
    pub panta_order_id: Option<String>,
    pub tx_signature: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Load one order; None if the id does not exist.
pub async fn get_order(
    client: &Client,
    order_id: Uuid,
) -> Result<Option<OrderRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT o.id, o.user_id, o.market_id, m.title, o.side, o.amount_usdc,
                    o.expected_shares, o.panta_quote_id, o.panta_order_id,
                    o.tx_signature, o.status, o.error_code, o.created_at, o.updated_at
             FROM orders o LEFT JOIN markets m ON m.id = o.market_id
             WHERE o.id = $1",
            &[&order_id],
        )
        .await?;
    Ok(row.map(|r| OrderRow {
        id: r.get(0),
        user_id: r.get(1),
        market_id: r.get(2),
        market_title: r.get(3),
        side: r.get(4),
        amount_usdc: r.get(5),
        expected_shares: r.get(6),
        panta_quote_id: r.get(7),
        panta_order_id: r.get(8),
        tx_signature: r.get(9),
        status: r.get(10),
        error_code: r.get(11),
        created_at: r.get(12),
        updated_at: r.get(13),
    }))
}

/// Order built at Panta + assembled into a transaction by us.
pub async fn mark_built(
    client: &Client,
    order_id: Uuid,
    panta_order_id: &str,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE orders SET status = 'built', panta_order_id = $2, updated_at = now()
             WHERE id = $1",
            &[&order_id, &panta_order_id],
        )
        .await?;
    Ok(())
}

/// Link an order to the basket purchase it belongs to (legs of a basket).
pub async fn link_to_purchase(
    client: &Client,
    order_id: Uuid,
    purchase_id: Uuid,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE orders SET basket_purchase_id = $2 WHERE id = $1",
            &[&order_id, &purchase_id],
        )
        .await?;
    Ok(())
}

/// Signed by the user and broadcast; awaiting confirmation.
pub async fn mark_submitted(
    client: &Client,
    order_id: Uuid,
    signature: &str,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE orders SET status = 'submitted', tx_signature = $2, updated_at = now()
             WHERE id = $1",
            &[&order_id, &signature],
        )
        .await?;
    Ok(())
}

/// Terminal state transitions during confirmation.
pub async fn set_status(
    client: &Client,
    order_id: Uuid,
    status: &str,
    error_code: Option<&str>,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE orders SET status = $2, error_code = $3, updated_at = now()
             WHERE id = $1",
            &[&order_id, &status, &error_code],
        )
        .await?;
    Ok(())
}

/// Record the transaction signature in the shared transactions table.
pub async fn insert_transaction(
    client: &Client,
    signature: &str,
    user_id: Uuid,
    kind: &str, // order | basket | claim
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO transactions (signature, user_id, kind, status)
             VALUES ($1, $2, $3, 'submitted')
             ON CONFLICT (signature) DO NOTHING",
            &[&signature, &user_id, &kind],
        )
        .await?;
    Ok(())
}

/// Mark a stored transaction as confirmed/failed.
pub async fn update_transaction_status(
    client: &Client,
    signature: &str,
    status: &str,
    raw_error: Option<&str>,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE transactions SET status = $2, confirmed_at = CASE
                 WHEN $2 = 'confirmed' THEN now() ELSE confirmed_at END,
                 raw_error = $3
             WHERE signature = $1",
            &[&signature, &status, &raw_error],
        )
        .await?;
    Ok(())
}
