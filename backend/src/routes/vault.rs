//! Tokenized baskets (ADR 0002): the `basket_vault` program behind a basket
//! template. Same guarantees as every other trade — the server builds,
//! validates and simulates; the wallet signs; submit only accepts the exact
//! bytes we built (`settlement`).
//!
//! Admin: `POST /v1/admin/baskets/{id}/tokenize | vault-close | vault-settle`
//! Users: `GET /v1/vault/baskets/{slug}`,
//!        `POST /v1/vault/baskets/{template_id}/deposit | redeem`,
//!        `POST /v1/vault/txs/{tx_id}/submit`, `GET /v1/vault/txs/{tx_id}`

use std::str::FromStr;

use axum::{
    extract::{Path, State},
    Json,
};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use uuid::Uuid;

use crate::auth::{AdminUser, AuthUser};
use crate::domain::{
    Side, SubmitRequest, TxSummary, VaultBasketView, VaultBuildResponse, VaultDepositRequest,
    VaultLegView, VaultRedeemRequest, VaultTxView,
};
use crate::error::AppError;
use crate::panta;
use crate::queries;
use crate::routes::orders::encode_tx;
use crate::settlement;
use crate::solana_tx::AssembledTx;
use crate::state::AppState;
use crate::vault::{self, VaultAddresses, VaultLeg};

/// Without lookup tables a deposit fits one transaction for up to 3 legs
/// (15 shared accounts + 4 per leg).
const MAX_VAULT_LEGS: usize = 3;
const MIN_DEPOSIT: i64 = 1_000_000;
const DEPOSIT_CU: u32 = 600_000;
const ADMIN_CU: u32 = 200_000;

// --- helpers -----------------------------------------------------------------

struct Ctx {
    program: Pubkey,
    usdc: Pubkey,
}

fn ctx(state: &AppState) -> Result<Ctx, AppError> {
    let program = state.config.vault_program_id.as_deref().ok_or_else(|| {
        AppError::bad_request("VAULT_DISABLED", "tokenized baskets are not configured")
    })?;
    if state.solana.is_mock() {
        return Err(AppError::bad_request(
            "VAULT_DISABLED",
            "tokenized baskets need a real Solana RPC (PANTA_MOCK=false)",
        ));
    }
    Ok(Ctx {
        program: parse_pk(program)?,
        usdc: parse_pk(&state.config.usdc_mint)?,
    })
}

fn parse_pk(s: &str) -> Result<Pubkey, AppError> {
    Pubkey::from_str(s)
        .map_err(|_| AppError::bad_request("INVALID_PUBKEY", format!("{s} is not a pubkey")))
}

fn legs_of(row: &queries::vault::VaultBasketRow) -> Result<Vec<VaultLeg>, AppError> {
    serde_json::from_value(row.legs_json.clone())
        .map_err(|_| AppError::internal("stored vault legs are malformed"))
}

fn addresses_of(c: &Ctx, row: &queries::vault::VaultBasketRow) -> VaultAddresses {
    vault::addresses(&c.program, row.onchain_id as u64, &c.usdc)
}

/// Assemble + validate + simulate one transaction for `payer`.
async fn prepare(
    state: &AppState,
    payer: &Pubkey,
    ixs: Vec<Instruction>,
    cu: u32,
) -> Result<AssembledTx, AppError> {
    let assembled = state.solana.assemble(payer, vec![ixs], None, cu).await?;
    state.solana.validate(&assembled.tx, payer, None)?;
    state.solana.simulate(&assembled.tx).await?;
    Ok(assembled)
}

/// Persist a built vault tx and shape the build response.
#[allow(clippy::too_many_arguments)]
async fn record(
    state: &AppState,
    user_id: Uuid,
    template_id: Uuid,
    kind: &str,
    amount: Option<i64>,
    tokens: Option<i64>,
    assembled: &AssembledTx,
    summary: TxSummary,
    expected: Option<u64>,
) -> Result<Json<VaultBuildResponse>, AppError> {
    let client = state.db.get().await?;
    let tx_id = queries::vault::insert_tx(
        &client,
        user_id,
        template_id,
        kind,
        amount,
        tokens,
        &assembled.message_bytes(),
        assembled.expiry(),
    )
    .await?;
    Ok(Json(VaultBuildResponse {
        tx_id,
        transactions: vec![encode_tx(&assembled.tx)?],
        summary,
        expected_tokens: expected.map(|e| e.to_string()),
        min_tokens_out: tokens.map(|t| t.to_string()),
    }))
}

fn summary(lines: Vec<String>, total: i64, fee: i64) -> TxSummary {
    TxSummary {
        summary_lines: lines,
        total_usdc: total.to_string(),
        our_fee_usdc: fee.to_string(),
        panta_fees_usdc: "0".into(),
        notices: vec![
            "Basket tokens can be redeemed for USDC after every market resolves.".into(),
            "Powered by Panta.".into(),
        ],
    }
}

// --- admin ---------------------------------------------------------------------

/// `POST /v1/admin/baskets/{id}/tokenize` — build the on-chain `create_basket`
/// for a published template. The admin wallet signs and pays rent.
pub async fn tokenize(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(template_id): Path<Uuid>,
) -> Result<Json<VaultBuildResponse>, AppError> {
    let c = ctx(&state)?;
    let client = state.db.get().await?;
    let template = queries::baskets::get_template(&client, template_id)
        .await?
        .ok_or_else(|| AppError::not_found("basket template not found"))?;
    if template.status != "published" {
        return Err(AppError::bad_request(
            "TEMPLATE_NOT_PUBLISHED",
            "publish the basket first",
        ));
    }
    if queries::vault::get_basket(&client, template_id)
        .await?
        .is_some()
    {
        return Err(AppError::bad_request(
            "ALREADY_TOKENIZED",
            "this basket already has a vault",
        ));
    }
    let rows = queries::baskets::get_legs(&client, template_id).await?;
    if rows.len() > MAX_VAULT_LEGS {
        return Err(AppError::bad_request(
            "TOO_MANY_LEGS",
            format!("tokenized baskets support up to {MAX_VAULT_LEGS} legs"),
        ));
    }
    drop(client);

    // Resolve each leg's Panta accounts from its own trade history.
    let mut legs = Vec::with_capacity(rows.len());
    let mut panta_fee = None;
    for row in &rows {
        let event = parse_pk(&row.market_id)?;
        let (authority, market_vault, fee) =
            vault::discover_market(&state.solana, &event, &c.usdc).await?;
        panta_fee.get_or_insert(fee);
        legs.push(VaultLeg {
            market_id: row.market_id.clone(),
            event: event.to_string(),
            authority: authority.to_string(),
            market_vault: market_vault.to_string(),
            side: if row.side == "yes" { 0 } else { 1 },
            weight_bps: row.weight_bps as u16,
        });
    }
    let panta_fee = panta_fee.ok_or_else(|| AppError::internal("basket has no legs"))?;

    // Deterministic on-chain id from the template id (fits a BIGINT).
    let onchain_id =
        u64::from_le_bytes(template_id.as_bytes()[..8].try_into().unwrap()) & (i64::MAX as u64);
    let a = vault::addresses(&c.program, onchain_id, &c.usdc);
    let admin_pk = parse_pk(&admin.user.wallet)?;
    let fee_wallet = parse_pk(&state.config.fee_wallet_pubkey)?;
    let fee_usdc = vault::ata(&fee_wallet, &c.usdc);
    let ixs = vec![
        vault::create_ata_idempotent(&admin_pk, &fee_wallet, &c.usdc),
        vault::create_basket_ix(
            &a,
            &admin_pk,
            onchain_id,
            &legs,
            state.config.app_fee_bps as u16,
            &fee_usdc,
            &c.usdc,
        )?,
    ];
    let assembled = prepare(&state, &admin_pk, ixs, ADMIN_CU).await?;

    let client = state.db.get().await?;
    queries::vault::insert_basket(
        &client,
        template_id,
        onchain_id as i64,
        &a.basket.to_string(),
        &a.vault.to_string(),
        &a.mint.to_string(),
        &panta_fee.to_string(),
        &serde_json::to_value(&legs).unwrap(),
    )
    .await?;
    drop(client);

    record(
        &state,
        admin.user.user_id,
        template_id,
        "create",
        None,
        None,
        &assembled,
        TxSummary {
            summary_lines: vec![format!(
                "Create the on-chain vault and basket token for \"{}\" ({} legs).",
                template.title,
                legs.len()
            )],
            total_usdc: "0".into(),
            our_fee_usdc: "0".into(),
            panta_fees_usdc: "0".into(),
            notices: vec!["You pay the rent for the vault accounts (SOL).".into()],
        },
        None,
    )
    .await
}

/// `POST /v1/admin/baskets/{id}/vault-close` and `/vault-settle`.
async fn lifecycle(
    state: AppState,
    admin: AdminUser,
    template_id: Uuid,
    kind: &str,
) -> Result<Json<VaultBuildResponse>, AppError> {
    let c = ctx(&state)?;
    let client = state.db.get().await?;
    let row = queries::vault::get_basket(&client, template_id)
        .await?
        .ok_or_else(|| AppError::not_found("this basket has no vault"))?;
    drop(client);
    let a = addresses_of(&c, &row);
    let admin_pk = parse_pk(&admin.user.wallet)?;
    let ix_name = if kind == "close" {
        "close_deposits"
    } else {
        "settle"
    };
    let assembled = prepare(
        &state,
        &admin_pk,
        vec![vault::admin_ix(ix_name, &a, &admin_pk)],
        ADMIN_CU,
    )
    .await?;
    let line = if kind == "close" {
        "Stop new deposits into this basket vault."
    } else {
        "Settle the basket vault: holders can redeem for USDC."
    };
    record(
        &state,
        admin.user.user_id,
        template_id,
        kind,
        None,
        None,
        &assembled,
        summary(vec![line.into()], 0, 0),
        None,
    )
    .await
}

pub async fn close(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(template_id): Path<Uuid>,
) -> Result<Json<VaultBuildResponse>, AppError> {
    lifecycle(state, admin, template_id, "close").await
}

pub async fn settle(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(template_id): Path<Uuid>,
) -> Result<Json<VaultBuildResponse>, AppError> {
    lifecycle(state, admin, template_id, "settle").await
}

// --- users ---------------------------------------------------------------------

/// `GET /v1/vault/baskets/{slug}` — vault info + on-chain NAV.
pub async fn basket(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<VaultBasketView>, AppError> {
    let client = state.db.get().await?;
    let row = queries::vault::get_basket_by_slug(&client, &slug)
        .await?
        .ok_or_else(|| AppError::not_found("this basket is not tokenized"))?;
    let legs = legs_of(&row)?;
    let mut leg_views = Vec::with_capacity(legs.len());
    for leg in &legs {
        let title = queries::markets::get_market(&client, &leg.market_id)
            .await?
            .map(|m| m.title);
        leg_views.push(VaultLegView {
            market_id: leg.market_id.clone(),
            market_title: title,
            side: if leg.side == 0 { Side::Yes } else { Side::No },
            weight_bps: leg.weight_bps as i32,
        });
    }
    drop(client);

    // NAV is best-effort: a down RPC must not break the page.
    let nav = match ctx(&state) {
        Ok(c) if row.status != "pending" => {
            vault::read_nav(&state.solana, &addresses_of(&c, &row), &legs)
                .await
                .ok()
        }
        _ => None,
    };
    Ok(Json(VaultBasketView {
        template_id: row.template_id,
        slug: row.slug,
        title: row.title,
        status: row.status,
        mint: row.mint,
        vault: row.vault_pda,
        legs: leg_views,
        nav_usdc: nav.map(|(n, _)| n.to_string()),
        supply: nav.map(|(_, s)| s.to_string()),
        nav_per_token: nav.and_then(|(n, s)| {
            (s > 0).then(|| format!("{:.4}", n as f64 / s as f64)) // display only
        }),
    }))
}

/// `POST /v1/vault/baskets/{template_id}/deposit` — simulate once to learn
/// the exact tokens minted, then build the real tx with
/// `min_tokens_out = expected × (1 − slippage)`.
pub async fn deposit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(template_id): Path<Uuid>,
    Json(req): Json<VaultDepositRequest>,
) -> Result<Json<VaultBuildResponse>, AppError> {
    let c = ctx(&state)?;
    let amount: i64 = req
        .amount_usdc
        .parse()
        .map_err(|_| AppError::bad_request("INVALID_AMOUNT", "amount must be an integer string"))?;
    if !(MIN_DEPOSIT..=1_000_000_000).contains(&amount) {
        return Err(AppError::bad_request(
            "AMOUNT_OUT_OF_RANGE",
            "deposit 1–1000 USDC",
        ));
    }
    let slippage = req
        .slippage_bps
        .unwrap_or(state.config.default_slippage_bps as i32);
    if !(1..=1_000).contains(&slippage) {
        return Err(AppError::bad_request(
            "INVALID_SLIPPAGE",
            "slippage_bps must be 1–1000",
        ));
    }
    let client = state.db.get().await?;
    let row = queries::vault::get_basket(&client, template_id)
        .await?
        .ok_or_else(|| AppError::not_found("this basket is not tokenized"))?;
    drop(client);
    if row.status != "live" {
        return Err(AppError::bad_request(
            "VAULT_NOT_LIVE",
            "this basket vault is not open for deposits",
        ));
    }
    let legs = legs_of(&row)?;
    let a = addresses_of(&c, &row);
    let user_pk = parse_pk(&user.wallet)?;
    let fee_usdc = vault::ata(&parse_pk(&state.config.fee_wallet_pubkey)?, &c.usdc);
    let panta_fee = parse_pk(&row.panta_fee_usdc)?;
    let ixs = |min_out: u64| -> Result<Vec<Instruction>, AppError> {
        Ok(vec![
            vault::create_ata_idempotent(&user_pk, &user_pk, &a.mint),
            vault::deposit_ix(
                &a,
                &user_pk,
                &legs,
                &fee_usdc,
                &panta_fee,
                &c.usdc,
                amount as u64,
                min_out,
            )?,
        ])
    };

    // 1. Probe: what would this deposit mint right now?
    let probe = state
        .solana
        .assemble(&user_pk, vec![ixs(1)?], None, DEPOSIT_CU)
        .await?;
    let user_basket = vault::ata(&user_pk, &a.mint);
    let before = state
        .solana
        .accounts_data(&[user_basket])
        .await?
        .remove(0)
        .and_then(|d| {
            d.get(64..72)
                .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
        })
        .unwrap_or(0);
    let after = state
        .solana
        .simulate_token_balance(&probe.tx, &user_basket)
        .await?;
    let expected = after.saturating_sub(before);
    let min_out = (expected as u128 * (10_000 - slippage as u128) / 10_000).max(1) as u64;

    // 2. The real transaction, with the slippage floor baked in.
    let assembled = prepare(&state, &user_pk, ixs(min_out)?, DEPOSIT_CU).await?;
    let fee = amount * state.config.app_fee_bps as i64 / 10_000;
    record(
        &state,
        user.user_id,
        template_id,
        "deposit",
        Some(amount),
        Some(min_out as i64),
        &assembled,
        summary(
            vec![format!(
                "You are depositing {} USDC into \"{}\" and receive about {} basket tokens.",
                panta::usdc_base_to_human(amount),
                row.title,
                panta::usdc_base_to_human(expected as i64),
            )],
            amount,
            fee,
        ),
        Some(expected),
    )
    .await
}

/// `POST /v1/vault/baskets/{template_id}/redeem` — burn tokens for USDC
/// (only once the basket is settled).
pub async fn redeem(
    State(state): State<AppState>,
    user: AuthUser,
    Path(template_id): Path<Uuid>,
    Json(req): Json<VaultRedeemRequest>,
) -> Result<Json<VaultBuildResponse>, AppError> {
    let c = ctx(&state)?;
    let tokens: i64 = req.tokens.parse().ok().filter(|t| *t > 0).ok_or_else(|| {
        AppError::bad_request("INVALID_AMOUNT", "tokens must be a positive integer string")
    })?;
    let client = state.db.get().await?;
    let row = queries::vault::get_basket(&client, template_id)
        .await?
        .ok_or_else(|| AppError::not_found("this basket is not tokenized"))?;
    drop(client);
    if row.status != "settled" {
        return Err(AppError::bad_request(
            "VAULT_NOT_SETTLED",
            "redeem opens once every market resolves",
        ));
    }
    let a = addresses_of(&c, &row);
    let user_pk = parse_pk(&user.wallet)?;
    let ixs = vec![
        vault::create_ata_idempotent(&user_pk, &user_pk, &c.usdc),
        vault::redeem_ix(&a, &user_pk, &c.usdc, tokens as u64)?,
    ];
    let assembled = prepare(&state, &user_pk, ixs, ADMIN_CU).await?;
    record(
        &state,
        user.user_id,
        template_id,
        "redeem",
        None,
        Some(tokens),
        &assembled,
        summary(
            vec![format!(
                "You are redeeming {} basket tokens of \"{}\" for your share of the vault's USDC.",
                panta::usdc_base_to_human(tokens),
                row.title
            )],
            0,
            0,
        ),
        None,
    )
    .await
}

/// `POST /v1/vault/txs/{tx_id}/submit` — same integrity rules as all trades.
pub async fn submit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(tx_id): Path<Uuid>,
    Json(req): Json<SubmitRequest>,
) -> Result<Json<VaultTxView>, AppError> {
    if req.signed_txs.len() != 1 {
        return Err(AppError::bad_request(
            "INVALID_REQUEST",
            "exactly one signed transaction",
        ));
    }
    let (signed, signature) = {
        let client = state.db.get().await?;
        let vtx = queries::vault::get_tx(&client, tx_id)
            .await?
            .filter(|t| t.user_id == user.user_id)
            .ok_or_else(|| AppError::not_found("vault transaction not found"))?;
        let signed = settlement::decode_signed(&req.signed_txs[0])?;
        let signature = settlement::verify_signed(&signed, &vtx.built_message)?;
        settlement::record_submission(
            &client,
            "vault",
            tx_id,
            user.user_id,
            &signature,
            vtx.last_valid_block_height,
        )
        .await?;
        (signed, signature)
    };
    settlement::broadcast_and_settle(&state, &signed, &signature).await?;
    tx_view(&state, user.user_id, tx_id).await
}

/// `GET /v1/vault/txs/{tx_id}`
pub async fn tx_status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(tx_id): Path<Uuid>,
) -> Result<Json<VaultTxView>, AppError> {
    tx_view(&state, user.user_id, tx_id).await
}

async fn tx_view(
    state: &AppState,
    user_id: Uuid,
    tx_id: Uuid,
) -> Result<Json<VaultTxView>, AppError> {
    let client = state.db.get().await?;
    let t = queries::vault::get_tx(&client, tx_id)
        .await?
        .filter(|t| t.user_id == user_id)
        .ok_or_else(|| AppError::not_found("vault transaction not found"))?;
    Ok(Json(VaultTxView {
        tx_id: t.id,
        kind: t.kind,
        status: t.status,
        amount_usdc: t.amount_usdc.map(|v| v.to_string()),
        tokens: t.tokens.map(|v| v.to_string()),
        tx_signature: t.tx_signature,
        error_code: t.error_code,
        created_at: t.created_at,
    }))
}

/// Portfolio section for tokenized baskets (best effort: chain reads that
/// fail leave `value_usdc` empty instead of failing the whole portfolio).
pub async fn holdings(
    state: &AppState,
    user_id: Uuid,
    wallet: &str,
) -> Result<Vec<crate::domain::PortfolioVaultHolding>, AppError> {
    let client = state.db.get().await?;
    let rows = queries::vault::user_deposits(&client, user_id).await?;
    drop(client);
    let Ok(c) = ctx(state) else {
        return Ok(Vec::new());
    };
    let user = parse_pk(wallet)?;
    let mut out = Vec::with_capacity(rows.len());
    for (row, deposited) in rows {
        let a = addresses_of(&c, &row);
        let balance = state
            .solana
            .accounts_data(&[vault::ata(&user, &a.mint)])
            .await
            .ok()
            .and_then(|mut d| d.remove(0))
            .and_then(|d| {
                d.get(64..72)
                    .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            })
            .unwrap_or(0);
        let value = match legs_of(&row) {
            Ok(legs) => vault::read_nav(&state.solana, &a, &legs)
                .await
                .ok()
                .filter(|(_, supply)| *supply > 0)
                .map(|(nav, supply)| (balance as u128 * nav as u128 / supply as u128) as u64),
            Err(_) => None,
        };
        out.push(crate::domain::PortfolioVaultHolding {
            template_id: row.template_id,
            template_slug: row.slug,
            template_title: row.title,
            status: row.status,
            mint: row.mint,
            tokens: balance.to_string(),
            value_usdc: value.map(|v| v.to_string()),
            cost_basis_usdc: deposited.to_string(),
        });
    }
    Ok(out)
}
