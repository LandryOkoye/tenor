//! Basket queries: templates, legs, quotes, purchases (brief §10).

use chrono::{DateTime, Utc};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::domain::{BasketLegView, Side};

// --- Templates --------------------------------------------------------------

/// Create a draft template with its legs. Legs are replaced wholesale on
/// update, so they live in their own table keyed by template.
#[allow(clippy::too_many_arguments)] // mirrors the table columns
pub async fn create_template(
    client: &Client,
    slug: &str,
    title: &str,
    description: &str,
    category: Option<&str>,
    cover_url: Option<&str>,
    min_buy_usdc: i64,
    max_buy_usdc: i64,
    created_by: &str,
) -> Result<Uuid, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO basket_templates (slug, title, description, category, cover_url,
                                           min_buy_usdc, max_buy_usdc, created_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id",
            &[
                &slug,
                &title,
                &description,
                &category,
                &cover_url,
                &min_buy_usdc,
                &max_buy_usdc,
                &created_by,
            ],
        )
        .await?;
    Ok(row.get(0))
}

/// Replace all legs of a template (only allowed while draft).
/// Takes a mutable client because legs are written in one transaction.
pub async fn set_legs(
    client: &mut deadpool_postgres::Client,
    template_id: Uuid,
    legs: &[(String, Side, i32)], // (market_id, side, weight_bps)
) -> Result<(), tokio_postgres::Error> {
    // Delete + insert in one transaction: legs are a set, not a patch.
    let tx = client.transaction().await?;
    tx.execute(
        "DELETE FROM basket_legs WHERE template_id = $1",
        &[&template_id],
    )
    .await?;
    for (i, (market_id, side, weight)) in legs.iter().enumerate() {
        tx.execute(
            "INSERT INTO basket_legs (template_id, market_id, side, weight_bps, position)
             VALUES ($1, $2, $3, $4, $5)",
            &[&template_id, market_id, &side.as_str(), weight, &(i as i32)],
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// One leg row joined with its market (title, prices, close time, tradability).
pub struct LegRow {
    pub market_id: String,
    pub market_title: Option<String>,
    pub side: String,
    pub weight_bps: i32,
    pub current_price: Option<String>, // price of the CHOSEN side
    pub close_time: Option<DateTime<Utc>>,
    pub phase: Option<String>,
}

/// Load legs (market join) ordered by position, for display and quoting.
pub async fn get_legs(
    client: &Client,
    template_id: Uuid,
) -> Result<Vec<LegRow>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT l.market_id, m.title, l.side, l.weight_bps,
                    CASE WHEN l.side = 'yes' THEN m.yes_price ELSE m.no_price END AS leg_price,
                    m.close_time, m.phase
             FROM basket_legs l LEFT JOIN markets m ON m.id = l.market_id
             WHERE l.template_id = $1
             ORDER BY l.position",
            &[&template_id],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| LegRow {
            market_id: r.get(0),
            market_title: r.get(1),
            side: r.get(2),
            weight_bps: r.get(3),
            current_price: r.get(4),
            close_time: r.get(5),
            phase: r.get(6),
        })
        .collect())
}

impl LegRow {
    /// Is the leg's market still accepting primary buys?
    pub fn tradable(&self) -> bool {
        self.market_title.is_some() && self.phase.as_deref() == Some("primary")
    }

    /// DB row -> API view.
    pub fn to_view(&self) -> BasketLegView {
        BasketLegView {
            market_id: self.market_id.clone(),
            market_title: self
                .market_title
                .clone()
                .unwrap_or_else(|| self.market_id.clone()),
            side: if self.side == "yes" {
                Side::Yes
            } else {
                Side::No
            },
            weight_bps: self.weight_bps,
            current_price: self.current_price.clone(),
            close_time: self.close_time,
            tradable: self.tradable(),
        }
    }
}

/// The mutable, non-leg fields of a template.
pub struct TemplateRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: Option<String>,
    pub cover_url: Option<String>,
    pub status: String,
    pub min_buy_usdc: i64,
    pub max_buy_usdc: i64,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
}

pub async fn get_template(
    client: &Client,
    id: Uuid,
) -> Result<Option<TemplateRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, slug, title, description, category, cover_url, status,
                    min_buy_usdc, max_buy_usdc, created_by, created_at, published_at, closed_at
             FROM basket_templates WHERE id = $1",
            &[&id],
        )
        .await?;
    Ok(row.map(|r| TemplateRow {
        id: r.get(0),
        slug: r.get(1),
        title: r.get(2),
        description: r.get(3),
        category: r.get(4),
        cover_url: r.get(5),
        status: r.get(6),
        min_buy_usdc: r.get(7),
        max_buy_usdc: r.get(8),
        created_by: r.get(9),
        created_at: r.get(10),
        published_at: r.get(11),
        closed_at: r.get(12),
    }))
}

pub async fn get_template_by_slug(
    client: &Client,
    slug: &str,
) -> Result<Option<TemplateRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, slug, title, description, category, cover_url, status,
                    min_buy_usdc, max_buy_usdc, created_by, created_at, published_at, closed_at
             FROM basket_templates WHERE slug = $1",
            &[&slug],
        )
        .await?;
    Ok(row.map(|r| TemplateRow {
        id: r.get(0),
        slug: r.get(1),
        title: r.get(2),
        description: r.get(3),
        category: r.get(4),
        cover_url: r.get(5),
        status: r.get(6),
        min_buy_usdc: r.get(7),
        max_buy_usdc: r.get(8),
        created_by: r.get(9),
        created_at: r.get(10),
        published_at: r.get(11),
        closed_at: r.get(12),
    }))
}

/// All published templates (public list), newest first.
pub async fn list_published(client: &Client) -> Result<Vec<TemplateRow>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT id, slug, title, description, category, cover_url, status,
                    min_buy_usdc, max_buy_usdc, created_by, created_at, published_at, closed_at
             FROM basket_templates WHERE status = 'published'
             ORDER BY published_at DESC",
            &[],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| TemplateRow {
            id: r.get(0),
            slug: r.get(1),
            title: r.get(2),
            description: r.get(3),
            category: r.get(4),
            cover_url: r.get(5),
            status: r.get(6),
            min_buy_usdc: r.get(7),
            max_buy_usdc: r.get(8),
            created_by: r.get(9),
            created_at: r.get(10),
            published_at: r.get(11),
            closed_at: r.get(12),
        })
        .collect())
}

/// Status transitions: draft -> published -> closed -> settled.
pub async fn set_template_status(
    client: &Client,
    id: Uuid,
    status: &str,
) -> Result<(), tokio_postgres::Error> {
    let now_setter = match status {
        "published" => ", published_at = now()",
        "closed" => ", closed_at = now()",
        _ => "",
    };
    let sql = format!("UPDATE basket_templates SET status = $2{now_setter} WHERE id = $1");
    client.execute(&sql, &[&id, &status]).await?;
    Ok(())
}

/// Update the mutable template fields (PATCH semantics; NULL = unchanged).
#[allow(clippy::too_many_arguments)]
pub async fn update_template(
    client: &Client,
    id: Uuid,
    title: Option<&str>,
    description: Option<&str>,
    category: Option<&str>,
    cover_url: Option<&str>,
    min_buy_usdc: Option<i64>,
    max_buy_usdc: Option<i64>,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE basket_templates SET
                title       = COALESCE($2, title),
                description = COALESCE($3, description),
                category    = COALESCE($4, category),
                cover_url   = COALESCE($5, cover_url),
                min_buy_usdc = COALESCE($6, min_buy_usdc),
                max_buy_usdc = COALESCE($7, max_buy_usdc)
             WHERE id = $1",
            &[
                &id,
                &title,
                &description,
                &category,
                &cover_url,
                &min_buy_usdc,
                &max_buy_usdc,
            ],
        )
        .await?;
    Ok(())
}

/// Total USDC bought through a template (filled purchases only).
/// `sum()` is NULL when nothing was bought yet -> None.
pub async fn total_bought(
    client: &Client,
    template_id: Uuid,
) -> Result<Option<i64>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            // sum(bigint) is NUMERIC in Postgres; cast back to BIGINT so the
            // column maps to i64 (NULL stays NULL -> None).
            "SELECT sum(amount_usdc)::BIGINT FROM basket_purchases
             WHERE template_id = $1 AND status IN ('filled', 'active', 'partially_resolved', 'resolved', 'claimed')",
            &[&template_id],
        )
        .await?;
    // Note the double Option: row may be missing OR the sum may be NULL.
    // `and_then` peels the row-level Option; the column is Option<i64>.
    Ok(row.and_then(|r| r.get::<_, Option<i64>>(0)))
}

// --- Quotes & purchases --------------------------------------------------------

/// Persist a basket quote: per-leg Panta quote details as JSON, plus totals.
#[allow(clippy::too_many_arguments)] // mirrors the table columns
pub async fn insert_quote(
    client: &Client,
    template_id: Uuid,
    user_id: Uuid,
    amount_usdc: i64,
    fee_usdc: i64,
    slippage_bps: i32,
    legs_json: &serde_json::Value,
    expires_at: DateTime<Utc>,
) -> Result<Uuid, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO basket_quotes (template_id, user_id, amount_usdc, fee_usdc,
                                        slippage_bps, legs_json, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id",
            &[
                &template_id,
                &user_id,
                &amount_usdc,
                &fee_usdc,
                &slippage_bps,
                &legs_json,
                &expires_at,
            ],
        )
        .await?;
    Ok(row.get(0))
}

/// One stored quote (checked at build time).
pub struct BasketQuoteRow {
    pub id: Uuid,
    pub template_id: Uuid,
    pub user_id: Uuid,
    pub amount_usdc: i64,
    pub fee_usdc: i64,
    pub slippage_bps: i32,
    pub legs_json: serde_json::Value,
    pub expires_at: DateTime<Utc>,
}

pub async fn get_quote(
    client: &Client,
    quote_id: Uuid,
) -> Result<Option<BasketQuoteRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, template_id, user_id, amount_usdc, fee_usdc, slippage_bps,
                    legs_json, expires_at
             FROM basket_quotes WHERE id = $1",
            &[&quote_id],
        )
        .await?;
    Ok(row.map(|r| BasketQuoteRow {
        id: r.get(0),
        template_id: r.get(1),
        user_id: r.get(2),
        amount_usdc: r.get(3),
        fee_usdc: r.get(4),
        slippage_bps: r.get(5),
        legs_json: r.get(6),
        expires_at: r.get(7),
    }))
}

/// Create the purchase record when a quote is built into a transaction.
pub async fn insert_purchase(
    client: &Client,
    template_id: Uuid,
    user_id: Uuid,
    quote_id: Uuid,
    amount_usdc: i64,
    fee_usdc: i64,
) -> Result<Uuid, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO basket_purchases (template_id, user_id, quote_id, amount_usdc,
                                           fee_usdc, status)
             VALUES ($1, $2, $3, $4, $5, 'built')
             RETURNING id",
            &[&template_id, &user_id, &quote_id, &amount_usdc, &fee_usdc],
        )
        .await?;
    Ok(row.get(0))
}

/// Purchase row for status views.
pub struct PurchaseRow {
    pub id: Uuid,
    pub template_id: Uuid,
    pub template_slug: Option<String>,
    pub template_title: Option<String>,
    pub user_id: Uuid,
    pub amount_usdc: i64,
    pub fee_usdc: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn get_purchase(
    client: &Client,
    purchase_id: Uuid,
) -> Result<Option<PurchaseRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT p.id, p.template_id, t.slug, t.title, p.user_id, p.amount_usdc,
                    p.fee_usdc, p.status, p.created_at, p.updated_at
             FROM basket_purchases p LEFT JOIN basket_templates t ON t.id = p.template_id
             WHERE p.id = $1",
            &[&purchase_id],
        )
        .await?;
    Ok(row.map(|r| PurchaseRow {
        id: r.get(0),
        template_id: r.get(1),
        template_slug: r.get(2),
        template_title: r.get(3),
        user_id: r.get(4),
        amount_usdc: r.get(5),
        fee_usdc: r.get(6),
        status: r.get(7),
        created_at: r.get(8),
        updated_at: r.get(9),
    }))
}

pub async fn set_purchase_status(
    client: &Client,
    purchase_id: Uuid,
    status: &str,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE basket_purchases SET status = $2, updated_at = now() WHERE id = $1",
            &[&purchase_id, &status],
        )
        .await?;
    Ok(())
}

/// All leg orders of a purchase (positions view + status tracking).
pub async fn purchase_leg_orders(
    client: &Client,
    purchase_id: Uuid,
) -> Result<Vec<crate::queries::orders::OrderRow>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT o.id, o.user_id, o.market_id, m.title, o.side, o.amount_usdc,
                    o.expected_shares, o.panta_quote_id, o.panta_order_id,
                    o.tx_signature, o.status, o.error_code, o.created_at, o.updated_at
             FROM orders o LEFT JOIN markets m ON m.id = o.market_id
             WHERE o.basket_purchase_id = $1
             ORDER BY o.created_at",
            &[&purchase_id],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| crate::queries::orders::OrderRow {
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
        })
        .collect())
}
