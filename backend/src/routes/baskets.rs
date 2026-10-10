//! Basket trading endpoints (brief §7.3, §9): the quote → build → sign →
//! submit flow for buying a whole curated bundle in ONE transaction.
//!
//! Money flow per basket purchase:
//!   amount_usdc
//!     ├─ our fee (75 bps)  → fee instruction inside the signed tx
//!     └─ net ─→ split by leg weights (floor) → Panta primary buys
//!         └─ dust (rounding remainder) stays unspent in the wallet

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{Duration, Utc};
use futures::future::join_all;
use std::str::FromStr;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::basket;
use crate::domain::{
    BasketBuildResponse, BasketQuote, BasketQuoteLeg, BasketQuoteRequest, BasketStatus,
    BasketTemplateDetail, BasketTemplateSummary, LegStatus, PurchaseLegView, PurchaseStatus, Side,
    SubmitRequest, TxSummary,
};
use crate::error::AppError;
use crate::panta;
use crate::queries;
use crate::routes::orders::encode_tx;
use crate::settlement;
use crate::state::AppState;

/// Safety margin subtracted from the earliest leg expiry when we tell the
/// user how long the whole-basket quote is valid (brief §7.3 step 4).
const QUOTE_EXPIRY_MARGIN_SECS: i64 = 15;
/// Highest slippage tolerance a user may pick (10%).
const MAX_SLIPPAGE_BPS: i32 = 1_000;

/// `GET /v1/baskets` — all published templates with live numbers.
pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<BasketTemplateSummary>>, AppError> {
    let client = state.db.get().await?;
    let templates = queries::baskets::list_published(&client).await?;

    let mut out = Vec::with_capacity(templates.len());
    for t in templates {
        let legs = queries::baskets::get_legs(&client, t.id).await?;
        let views: Vec<_> = legs.iter().map(|l| l.to_view()).collect();
        let soonest = legs.iter().filter_map(|l| l.close_time).min();
        out.push(BasketTemplateSummary {
            id: t.id,
            slug: t.slug,
            title: t.title,
            description: t.description,
            category: t.category,
            cover_url: t.cover_url,
            status: BasketStatus::Published,
            leg_count: legs.len() as i32,
            min_buy_usdc: t.min_buy_usdc.to_string(),
            max_buy_usdc: t.max_buy_usdc.to_string(),
            index_price: basket::index_price(&views),
            closes_in_seconds: soonest.map(|c| (c - Utc::now()).num_seconds()),
            total_bought_usdc: queries::baskets::total_bought(&client, t.id)
                .await?
                .map(|v| v.to_string()),
            published_at: t.published_at,
        });
    }
    Ok(Json(out))
}

/// `GET /v1/baskets/{slug}` — one template with live leg prices.
pub async fn detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<BasketTemplateDetail>, AppError> {
    let client = state.db.get().await?;
    let t = queries::baskets::get_template_by_slug(&client, &slug)
        .await?
        .ok_or_else(|| AppError::not_found("basket not found"))?;
    let legs = queries::baskets::get_legs(&client, t.id).await?;
    let views: Vec<_> = legs.iter().map(|l| l.to_view()).collect();

    Ok(Json(BasketTemplateDetail {
        id: t.id,
        slug: t.slug,
        title: t.title,
        description: t.description,
        category: t.category,
        cover_url: t.cover_url,
        status: parse_status(&t.status),
        min_buy_usdc: t.min_buy_usdc.to_string(),
        max_buy_usdc: t.max_buy_usdc.to_string(),
        index_price: basket::index_price(&views),
        legs: views,
        total_bought_usdc: queries::baskets::total_bought(&client, t.id)
            .await?
            .map(|v| v.to_string()),
        published_at: t.published_at,
    }))
}

/// `POST /v1/baskets/{id}/quote` — price a whole basket (brief §7.3).
///
/// Allocates the amount by weights (fee off the top), asks Panta to quote
/// every leg in parallel, and stores one basket quote with the earliest
/// leg expiry (minus a safety margin) as its lifetime.
pub async fn quote(
    State(state): State<AppState>,
    user: AuthUser,
    Path(template_id): Path<Uuid>,
    Json(req): Json<BasketQuoteRequest>,
) -> Result<Json<BasketQuote>, AppError> {
    // 1. Load template + legs; must be published and fully tradable.
    let client = state.db.get().await?;
    let t = queries::baskets::get_template(&client, template_id)
        .await?
        .ok_or_else(|| AppError::not_found("basket not found"))?;
    if t.status != "published" {
        return Err(AppError::bad_request(
            "BASKET_NOT_PUBLISHED",
            "this basket is not open for purchases",
        ));
    }
    let legs = queries::baskets::get_legs(&client, template_id).await?;
    if legs.is_empty() {
        return Err(AppError::internal("basket has no legs"));
    }
    if let Some(bad) = legs.iter().find(|l| !l.tradable()) {
        return Err(AppError::bad_request(
            "LEG_NOT_TRADABLE",
            format!("leg {} is not tradable anymore", bad.market_id),
        ));
    }

    // 2. Validate the amount against the template's buy range.
    let amount = parse_amount(&req.amount_usdc)?;
    if amount < t.min_buy_usdc || amount > t.max_buy_usdc {
        return Err(AppError::bad_request(
            "AMOUNT_OUT_OF_RANGE",
            format!(
                "amount must be between {} and {} USDC",
                panta::usdc_base_to_human(t.min_buy_usdc),
                panta::usdc_base_to_human(t.max_buy_usdc)
            ),
        ));
    }

    // Slippage tolerance: 0.01%–10% (Panta caps at 50%; above 10% a
    // "basket" stops being a sensible purchase).
    let slippage_bps = req
        .slippage_bps
        .unwrap_or(state.config.default_slippage_bps as i32);
    if !(1..=MAX_SLIPPAGE_BPS).contains(&slippage_bps) {
        return Err(AppError::bad_request(
            "INVALID_SLIPPAGE",
            format!("slippage_bps must be between 1 and {MAX_SLIPPAGE_BPS}"),
        ));
    }

    // 3. Allocate: fee off the top, then weights (floor; dust unspent).
    let weights: Vec<i32> = legs.iter().map(|l| l.weight_bps).collect();
    let (allocations, fee_base, _dust) =
        basket::allocate(amount, state.config.app_fee_bps as i64, &weights);

    // 4. Quote every leg at Panta in parallel (one future per leg).
    let quote_futures = legs.iter().zip(&allocations).map(|(leg, alloc)| {
        let panta = state.panta.clone();
        let wallet = user.wallet.clone();
        let market_id = leg.market_id.clone();
        let side = leg.side.clone();
        let title = leg
            .market_title
            .clone()
            .unwrap_or_else(|| leg.market_id.clone());
        let weight = leg.weight_bps;
        let alloc_human = panta::usdc_base_to_human(*alloc);
        async move {
            let result = panta
                .quote_primary(&panta::QuoteRequest {
                    wallet,
                    market_id: market_id.clone(),
                    side: side.clone(),
                    amount_usdc: alloc_human,
                    user_id: None,
                })
                .await;
            (result, market_id, title, side, weight, *alloc)
        }
    });
    let results = join_all(quote_futures).await;

    // 5. Fold the leg quotes into one basket quote; fail if any leg failed.
    let mut leg_views = Vec::with_capacity(results.len());
    let mut total_panta_fees: i64 = 0;
    let mut soonest_expiry: Option<chrono::DateTime<Utc>> = None;
    let mut legs_json = Vec::with_capacity(results.len());
    for (result, market_id, title, side, weight_bps, alloc) in results {
        let q = result?; // any leg failure fails the whole quote (atomic UX)
        let shares = panta::usdc_human_to_base(&q.shares).unwrap_or(0);
        let fee = panta::usdc_human_to_base(&q.fee_usdc).unwrap_or(0);
        total_panta_fees += fee;
        soonest_expiry = Some(match soonest_expiry {
            Some(e) if e < q.expires_at => e,
            _ => q.expires_at,
        });
        legs_json.push(serde_json::json!({
            "market_id": market_id,
            "side": side,
            "weight_bps": weight_bps,
            "allocation_base": alloc,
            "panta_quote_id": q.quote_id,
            "shares_base": shares,
            "avg_price": q.avg_price,
            "fee_base": fee,
        }));
        leg_views.push(BasketQuoteLeg {
            market_id,
            market_title: title,
            side: if side == "yes" { Side::Yes } else { Side::No },
            weight_bps,
            allocation_usdc: alloc.to_string(),
            expected_shares: shares.to_string(),
            avg_price: q.avg_price,
            panta_fee_usdc: fee.to_string(),
            price_impact_bps: None,
        });
    }
    let expires_at = soonest_expiry.unwrap_or_else(|| Utc::now() + Duration::seconds(90))
        - Duration::seconds(QUOTE_EXPIRY_MARGIN_SECS);

    // 6. Persist and return.
    let quote_id = queries::baskets::insert_quote(
        &client,
        template_id,
        user.user_id,
        amount,
        fee_base,
        slippage_bps,
        &serde_json::Value::Array(legs_json),
        expires_at,
    )
    .await?;

    Ok(Json(BasketQuote {
        quote_id,
        template_id,
        amount_usdc: amount.to_string(),
        our_fee_usdc: fee_base.to_string(),
        total_panta_fees_usdc: total_panta_fees.to_string(),
        legs: leg_views,
        expires_at,
    }))
}

/// `POST /v1/baskets/quotes/{quoteId}/build` — turn a stored quote into ONE
/// unsigned transaction covering every leg plus our fee instruction.
pub async fn build(
    State(state): State<AppState>,
    user: AuthUser,
    Path(quote_id): Path<Uuid>,
) -> Result<Json<BasketBuildResponse>, AppError> {
    let client = state.db.get().await?;

    // 1. Load + own the quote; refuse expired quotes (client should requote).
    let q = queries::baskets::get_quote(&client, quote_id)
        .await?
        .filter(|q| q.user_id == user.user_id)
        .ok_or_else(|| AppError::not_found("quote not found"))?;
    if Utc::now() > q.expires_at {
        return Err(AppError::bad_request(
            "QUOTE_STALE",
            "quote expired; request a new one",
        ));
    }
    if queries::baskets::quote_already_built(&client, quote_id).await? {
        return Err(AppError::bad_request(
            "QUOTE_ALREADY_USED",
            "this quote was already built; request a new one",
        ));
    }
    let template = queries::baskets::get_template(&client, q.template_id)
        .await?
        .ok_or_else(|| AppError::internal("template vanished"))?;

    // 2. Parse the stored legs and build each one at Panta.
    let legs = q
        .legs_json
        .as_array()
        .ok_or_else(|| AppError::internal("stored quote legs are malformed"))?;
    let build_futures = legs.iter().map(|leg| {
        let panta = state.panta.clone();
        let wallet = user.wallet.clone();
        let quote_id = leg["panta_quote_id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let slippage = q.slippage_bps as u32;
        async move {
            panta
                .build_primary(&panta::BuildRequest {
                    quote_id,
                    wallet,
                    user_id: None,
                    max_slippage_bps: Some(slippage),
                })
                .await
        }
    });
    let builds = join_all(build_futures).await;

    // 3. Assemble ONE transaction: [compute budget] + [leg instructions...] + [fee].
    let payer = solana_sdk::pubkey::Pubkey::from_str(&user.wallet)
        .map_err(|_| AppError::bad_request("INVALID_WALLET", "wallet is not a valid pubkey"))?;
    let mut leg_instruction_sets = Vec::with_capacity(builds.len());
    let mut order_ids = Vec::with_capacity(builds.len());
    for b in builds {
        let b = b?;
        let mut ixs = Vec::with_capacity(b.instructions.len());
        for wire in &b.instructions {
            ixs.push(crate::solana_tx::SolanaCtx::parse_wire_instruction(wire)?);
        }
        leg_instruction_sets.push(ixs);
        order_ids.push(b.order_id.clone());
    }
    let assembled = state
        .solana
        .assemble(&payer, leg_instruction_sets, Some(q.fee_usdc), 400_000)
        .await?;

    // 4. Funds-protection validation (with the fee audit) + simulation.
    state
        .solana
        .validate(&assembled.tx, &payer, Some(q.fee_usdc))?;
    state.solana.simulate(&assembled.tx).await?;

    // 5. Persist atomically: purchase (with the exact message bytes the user
    //    will sign) + one order row per leg, all linked together.
    client.batch_execute("BEGIN").await?;
    let persisted: Result<Uuid, AppError> = async {
        let purchase_id = queries::baskets::insert_purchase(
            &client,
            q.template_id,
            user.user_id,
            q.id,
            q.amount_usdc,
            q.fee_usdc,
            &assembled.message_bytes(),
            assembled.expiry(),
        )
        .await?;
        for (leg, panta_order_id) in legs.iter().zip(&order_ids) {
            let market_id = leg["market_id"].as_str().unwrap_or_default();
            let side = leg["side"].as_str().unwrap_or("yes");
            let alloc = leg["allocation_base"].as_i64().unwrap_or(0);
            let shares = leg["shares_base"].as_i64();
            let panta_quote_id = leg["panta_quote_id"].as_str().unwrap_or_default();
            let order_id = queries::orders::insert_order(
                &client,
                user.user_id,
                market_id,
                side,
                alloc,
                shares,
                panta_quote_id,
            )
            .await?;
            queries::orders::link_to_purchase(&client, order_id, purchase_id).await?;
            queries::orders::mark_built(&client, order_id, panta_order_id, None, None).await?;
        }
        Ok(purchase_id)
    }
    .await;
    let purchase_id = match persisted {
        Ok(id) => {
            client.batch_execute("COMMIT").await?;
            id
        }
        Err(e) => {
            client.batch_execute("ROLLBACK").await?;
            return Err(e);
        }
    };

    // 6. Human-readable disclosure before signing (brief §12).
    let n_legs = legs.len();
    let summary = TxSummary {
        summary_lines: vec![format!(
            "You are buying {n_legs} positions in \"{}\" for {} USDC.",
            template.title,
            panta::usdc_base_to_human(q.amount_usdc)
        )],
        total_usdc: q.amount_usdc.to_string(),
        our_fee_usdc: q.fee_usdc.to_string(),
        panta_fees_usdc: legs
            .iter()
            .filter_map(|l| l["fee_base"].as_i64())
            .sum::<i64>()
            .to_string(),
        notices: vec![
            "Each leg pays out separately when its market resolves.".to_string(),
            "You cannot sell before these markets resolve.".to_string(),
            "Powered by Panta.".to_string(),
        ],
    };

    Ok(Json(BasketBuildResponse {
        purchase_id,
        transactions: vec![encode_tx(&assembled.tx)?],
        summary,
        expires_at: q.expires_at,
    }))
}

/// `POST /v1/baskets/purchases/{id}/submit` — broadcast the signed basket tx
/// and settle statuses + Panta attribution for every leg.
///
/// Same guarantees as single orders: the signed tx must be byte-identical to
/// the one we built (so our fee instruction cannot be dropped), the
/// signature is recorded before broadcasting, and a still-pending tx is
/// finished by the `tx_confirm` worker job.
pub async fn submit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(purchase_id): Path<Uuid>,
    Json(req): Json<SubmitRequest>,
) -> Result<Json<crate::domain::BasketPurchaseView>, AppError> {
    if req.signed_txs.len() != 1 {
        return Err(AppError::bad_request(
            "INVALID_REQUEST",
            "a basket purchase (single transaction) needs exactly one signature",
        ));
    }

    let (signed, signature) = {
        let client = state.db.get().await?;
        let purchase = queries::baskets::get_purchase(&client, purchase_id)
            .await?
            .filter(|p| p.user_id == user.user_id)
            .ok_or_else(|| AppError::not_found("purchase not found"))?;
        if purchase.status != "built" {
            return Err(AppError::bad_request(
                "PURCHASE_NOT_BUILT",
                format!("purchase is in status '{}'", purchase.status),
            ));
        }
        let (built_message, expiry) = queries::baskets::purchase_build_info(&client, purchase_id)
            .await?
            .ok_or_else(|| AppError::internal("purchase has no built transaction"))?;

        // ONE tx means all-or-nothing; it must be exactly the one we built.
        let signed = settlement::decode_signed(&req.signed_txs[0])?;
        let signature = settlement::verify_signed(&signed, &built_message)?;

        settlement::record_submission(
            &client,
            "basket",
            purchase_id,
            user.user_id,
            &signature,
            expiry,
        )
        .await?;
        (signed, signature)
    };

    settlement::broadcast_and_settle(&state, &signed, &signature).await?;

    let client = state.db.get().await?;
    Ok(Json(purchase_view(&client, purchase_id).await?))
}

/// `GET /v1/baskets/purchases/{id}` — per-leg progress for the frontend.
pub async fn purchase_status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(purchase_id): Path<Uuid>,
) -> Result<Json<crate::domain::BasketPurchaseView>, AppError> {
    let client = state.db.get().await?;

    // Ownership check: other users' purchases do not exist to you.
    let owned = queries::baskets::get_purchase(&client, purchase_id)
        .await?
        .is_some_and(|p| p.user_id == user.user_id);
    if !owned {
        return Err(AppError::not_found("purchase not found"));
    }

    let view = purchase_view(&client, purchase_id).await?;
    Ok(Json(view))
}

// --- helpers -----------------------------------------------------------------

fn parse_amount(s: &str) -> Result<i64, AppError> {
    s.parse().map_err(|_| {
        AppError::bad_request(
            "INVALID_AMOUNT",
            "amount must be an integer string of base units",
        )
    })
}

fn parse_status(s: &str) -> BasketStatus {
    match s {
        "published" => BasketStatus::Published,
        "closed" => BasketStatus::Closed,
        "settled" => BasketStatus::Settled,
        _ => BasketStatus::Draft,
    }
}

/// Build the full purchase view (per-leg status, current prices, claimable).
async fn purchase_view(
    client: &deadpool_postgres::Client,
    purchase_id: Uuid,
) -> Result<crate::domain::BasketPurchaseView, AppError> {
    let purchase = queries::baskets::get_purchase(client, purchase_id)
        .await?
        .ok_or_else(|| AppError::internal("purchase vanished"))?;

    let orders = queries::baskets::purchase_leg_orders(client, purchase_id).await?;
    let mut leg_views = Vec::with_capacity(orders.len());
    for leg in &orders {
        // Current side price for display.
        // Current side price for display: pick the leg's side from the row.
        let price = queries::markets::get_market(client, &leg.market_id)
            .await?
            .and_then(|m| {
                if leg.side == "yes" {
                    m.yes_price
                } else {
                    m.no_price
                }
            });
        leg_views.push(PurchaseLegView {
            order_id: leg.id,
            market_id: leg.market_id.clone(),
            market_title: leg
                .market_title
                .clone()
                .unwrap_or_else(|| leg.market_id.clone()),
            side: if leg.side == "yes" {
                Side::Yes
            } else {
                Side::No
            },
            amount_usdc: leg.amount_usdc.to_string(),
            shares: leg.expected_shares.map(|s| s.to_string()),
            status: parse_leg_status(&leg.status),
            tx_signature: leg.tx_signature.clone(),
            error_code: leg.error_code.clone(),
            current_price: price,
            claimable: false, // filled in by the portfolio/resolution step
        });
    }

    Ok(crate::domain::BasketPurchaseView {
        id: purchase.id,
        template_id: purchase.template_id,
        template_slug: purchase.template_slug.unwrap_or_default(),
        template_title: purchase.template_title.unwrap_or_default(),
        amount_usdc: purchase.amount_usdc.to_string(),
        fee_usdc: purchase.fee_usdc.to_string(),
        status: parse_purchase_status(&purchase.status),
        legs: leg_views,
        created_at: purchase.created_at,
        updated_at: purchase.updated_at,
    })
}

fn parse_leg_status(s: &str) -> LegStatus {
    match s {
        "filled" => LegStatus::Filled,
        "failed" => LegStatus::Failed,
        "resolved_win" => LegStatus::ResolvedWin,
        "resolved_loss" => LegStatus::ResolvedLoss,
        "claimed" => LegStatus::Claimed,
        _ => LegStatus::Pending,
    }
}

fn parse_purchase_status(s: &str) -> PurchaseStatus {
    match s {
        "built" => PurchaseStatus::Built,
        "submitted" => PurchaseStatus::Submitted,
        "confirmed" => PurchaseStatus::Confirmed,
        "filled" => PurchaseStatus::Filled,
        "partial" => PurchaseStatus::Partial,
        "failed" => PurchaseStatus::Failed,
        "active" => PurchaseStatus::Active,
        "partially_resolved" => PurchaseStatus::PartiallyResolved,
        "resolved" => PurchaseStatus::Resolved,
        "claimed" => PurchaseStatus::Claimed,
        _ => PurchaseStatus::Quoted,
    }
}
