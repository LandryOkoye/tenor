//! Tokenized-basket queries: `vault_baskets` and `vault_txs` (migration 0007).

use chrono::{DateTime, Utc};
use tokio_postgres::Client;
use uuid::Uuid;

pub struct VaultBasketRow {
    pub template_id: Uuid,
    pub onchain_id: i64,
    pub mint: String,
    pub vault_pda: String,
    pub panta_fee_usdc: String,
    pub legs_json: serde_json::Value,
    pub status: String,
    pub slug: String,
    pub title: String,
}

const SELECT_BASKET: &str = "SELECT v.template_id, v.onchain_id, v.mint, v.vault_pda,
            v.panta_fee_usdc, v.legs_json, v.status, t.slug, t.title
     FROM vault_baskets v JOIN basket_templates t ON t.id = v.template_id";

fn basket_row(r: &tokio_postgres::Row) -> VaultBasketRow {
    VaultBasketRow {
        template_id: r.get(0),
        onchain_id: r.get(1),
        mint: r.get(2),
        vault_pda: r.get(3),
        panta_fee_usdc: r.get(4),
        legs_json: r.get(5),
        status: r.get(6),
        slug: r.get(7),
        title: r.get(8),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_basket(
    client: &Client,
    template_id: Uuid,
    onchain_id: i64,
    basket_pda: &str,
    vault_pda: &str,
    mint: &str,
    panta_fee_usdc: &str,
    legs_json: &serde_json::Value,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO vault_baskets (template_id, onchain_id, basket_pda, vault_pda, mint,
                                        panta_fee_usdc, legs_json)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (template_id) DO NOTHING",
            &[
                &template_id,
                &onchain_id,
                &basket_pda,
                &vault_pda,
                &mint,
                &panta_fee_usdc,
                legs_json,
            ],
        )
        .await?;
    Ok(())
}

pub async fn get_basket(
    client: &Client,
    template_id: Uuid,
) -> Result<Option<VaultBasketRow>, tokio_postgres::Error> {
    let sql = format!("{SELECT_BASKET} WHERE v.template_id = $1");
    Ok(client
        .query_opt(&sql, &[&template_id])
        .await?
        .map(|r| basket_row(&r)))
}

pub async fn get_basket_by_slug(
    client: &Client,
    slug: &str,
) -> Result<Option<VaultBasketRow>, tokio_postgres::Error> {
    let sql = format!("{SELECT_BASKET} WHERE t.slug = $1");
    Ok(client
        .query_opt(&sql, &[&slug])
        .await?
        .map(|r| basket_row(&r)))
}

pub async fn set_basket_status(
    client: &Client,
    template_id: Uuid,
    status: &str,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE vault_baskets SET status = $2 WHERE template_id = $1",
            &[&template_id, &status],
        )
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_tx(
    client: &Client,
    user_id: Uuid,
    template_id: Uuid,
    kind: &str,
    amount_usdc: Option<i64>,
    tokens: Option<i64>,
    built_message: &[u8],
    last_valid_block_height: Option<i64>,
) -> Result<Uuid, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO vault_txs (user_id, template_id, kind, amount_usdc, tokens,
                                    built_message, last_valid_block_height)
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
            &[
                &user_id,
                &template_id,
                &kind,
                &amount_usdc,
                &tokens,
                &built_message,
                &last_valid_block_height,
            ],
        )
        .await?;
    Ok(row.get(0))
}

pub struct VaultTxRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Uuid,
    pub kind: String,
    pub amount_usdc: Option<i64>,
    pub tokens: Option<i64>,
    pub built_message: Vec<u8>,
    pub last_valid_block_height: Option<i64>,
    pub tx_signature: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn get_tx(
    client: &Client,
    id: Uuid,
) -> Result<Option<VaultTxRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT id, user_id, template_id, kind, amount_usdc, tokens, built_message,
                    last_valid_block_height, tx_signature, status, error_code, created_at
             FROM vault_txs WHERE id = $1",
            &[&id],
        )
        .await?;
    Ok(row.map(|r| VaultTxRow {
        id: r.get(0),
        user_id: r.get(1),
        template_id: r.get(2),
        kind: r.get(3),
        amount_usdc: r.get(4),
        tokens: r.get(5),
        built_message: r.get(6),
        last_valid_block_height: r.get(7),
        tx_signature: r.get(8),
        status: r.get(9),
        error_code: r.get(10),
        created_at: r.get(11),
    }))
}

/// Move a vault tx `from` -> `to` (no-op otherwise). Returns true if moved.
pub async fn transition_tx(
    client: &Client,
    id: Uuid,
    from: &str,
    to: &str,
    signature: Option<&str>,
    error_code: Option<&str>,
) -> Result<bool, tokio_postgres::Error> {
    let n = client
        .execute(
            "UPDATE vault_txs SET status = $3, tx_signature = COALESCE($4, tx_signature),
                                  error_code = $5, updated_at = now()
             WHERE id = $1 AND status = $2",
            &[&id, &from, &to, &signature, &error_code],
        )
        .await?;
    Ok(n == 1)
}

/// Vault baskets a user deposited into, with their confirmed deposit total.
pub async fn user_deposits(
    client: &Client,
    user_id: Uuid,
) -> Result<Vec<(VaultBasketRow, i64)>, tokio_postgres::Error> {
    let sql = format!(
        "{SELECT_BASKET},
         LATERAL (SELECT sum(x.amount_usdc)::BIGINT AS deposited FROM vault_txs x
                  WHERE x.template_id = v.template_id AND x.user_id = $1
                    AND x.kind = 'deposit' AND x.status = 'confirmed') d
         WHERE d.deposited IS NOT NULL"
    );
    let sql = sql.replace("SELECT v.template_id", "SELECT d.deposited, v.template_id");
    Ok(client
        .query(&sql, &[&user_id])
        .await?
        .iter()
        .map(|r| {
            let deposited: i64 = r.get(0);
            let row = VaultBasketRow {
                template_id: r.get(1),
                onchain_id: r.get(2),
                mint: r.get(3),
                vault_pda: r.get(4),
                panta_fee_usdc: r.get(5),
                legs_json: r.get(6),
                status: r.get(7),
                slug: r.get(8),
                title: r.get(9),
            };
            (row, deposited)
        })
        .collect())
}
