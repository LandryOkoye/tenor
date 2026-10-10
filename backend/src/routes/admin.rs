//! Admin endpoints for basket curation (brief §7.9, §9).
//!
//! Only wallets on the ADMIN_WALLETS allowlist may call these (AdminUser
//! extractor). Flow: search markets -> create draft template with legs ->
//! fix warnings -> publish. Templates auto-close via the worker's
//! basket_guard job when legs stop being tradable.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AdminUser;
use crate::basket::{self, LegSpec};
use crate::domain::{AdminBasketView, AdminPantaMarket, CreateBasketRequest, UpdateBasketRequest};
use crate::error::AppError;
use crate::queries;
use crate::state::AppState;

/// `GET /v1/admin/panta-markets?q=` — search the local catalog (kept fresh by
/// the worker; bootstraps from Panta on an empty database).
pub async fn search_markets(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<AdminPantaMarket>>, AppError> {
    let _ = admin; // extractor already enforced the allowlist
    let client = state.db.get().await?;

    // Bootstrap on empty so a fresh dev environment is usable immediately.
    if queries::markets::count(&client).await? == 0 {
        drop(client);
        crate::routes::markets::sync_first_pages(&state, 3).await?;
    }
    let client = state.db.get().await?;

    let q = params.get("q").cloned();
    let (items, _) =
        queries::markets::list_markets(&client, q.as_deref(), None, None, None, 50).await?;

    Ok(Json(
        items
            .into_iter()
            .map(|m| AdminPantaMarket {
                id: m.id,
                title: m.title,
                category: m.category,
                status: m.status,
                close_time: m.close_time,
                volume_usdc: m.volume_usdc,
                yes_price: m.yes_price,
                no_price: m.no_price,
            })
            .collect(),
    ))
}

/// `POST /v1/admin/baskets` — create a draft template with legs.
pub async fn create(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(req): Json<CreateBasketRequest>,
) -> Result<Json<AdminBasketView>, AppError> {
    // Structural validation before touching the database.
    let specs = req
        .legs
        .iter()
        .map(|l| LegSpec {
            market_id: l.market_id.clone(),
            side: l.side,
            weight_bps: l.weight_bps,
        })
        .collect::<Vec<_>>();
    basket::validate_template(&specs, 2, state.config.max_basket_legs)
        .map_err(|e| AppError::bad_request("INVALID_TEMPLATE", e.to_string()))?;

    let min_buy = parse_money("min_buy_usdc", &req.min_buy_usdc)?;
    let max_buy = parse_money("max_buy_usdc", &req.max_buy_usdc)?;
    if min_buy <= 0 || max_buy < min_buy {
        return Err(AppError::bad_request(
            "INVALID_TEMPLATE",
            "min_buy must be > 0 and <= max_buy",
        ));
    }

    // Every leg's market must exist in our catalog (fetched once if new).
    for spec in &specs {
        crate::routes::markets::ensure_market(&state, &spec.market_id).await?;
    }
    let mut client = state.db.get().await?;

    let id = queries::baskets::create_template(
        &client,
        &req.slug,
        &req.title,
        &req.description,
        req.category.as_deref(),
        req.cover_url.as_deref(),
        min_buy,
        max_buy,
        &admin.user.wallet,
    )
    .await?;

    queries::baskets::set_legs(
        &mut client,
        id,
        &specs
            .iter()
            .map(|s| (s.market_id.clone(), s.side, s.weight_bps))
            .collect::<Vec<_>>(),
    )
    .await?;

    let view = admin_view(&state, &client, id).await?;
    Ok(Json(view))
}

/// `PATCH /v1/admin/baskets/{id}` — edit fields and/or replace legs
/// (only while the template is still a draft).
pub async fn update(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateBasketRequest>,
) -> Result<Json<AdminBasketView>, AppError> {
    let id = parse_id(&id)?;
    let mut client = state.db.get().await?;

    let template = queries::baskets::get_template(&client, id)
        .await?
        .ok_or_else(|| AppError::not_found("basket template not found"))?;
    if template.status != "draft" {
        return Err(AppError::bad_request(
            "TEMPLATE_NOT_DRAFT",
            "only draft templates can be edited",
        ));
    }

    // Buy range: parse strictly and keep 0 < min <= max after the patch.
    let min_buy = req
        .min_buy_usdc
        .as_deref()
        .map(|m| parse_money("min_buy_usdc", m))
        .transpose()?;
    let max_buy = req
        .max_buy_usdc
        .as_deref()
        .map(|m| parse_money("max_buy_usdc", m))
        .transpose()?;
    let (new_min, new_max) = (
        min_buy.unwrap_or(template.min_buy_usdc),
        max_buy.unwrap_or(template.max_buy_usdc),
    );
    if new_min <= 0 || new_max < new_min {
        return Err(AppError::bad_request(
            "INVALID_TEMPLATE",
            "min_buy must be > 0 and <= max_buy",
        ));
    }

    // Replace legs if provided — same validation as create.
    if let Some(legs) = &req.legs {
        for leg in legs {
            crate::routes::markets::ensure_market(&state, &leg.market_id).await?;
        }
        let specs = legs
            .iter()
            .map(|l| LegSpec {
                market_id: l.market_id.clone(),
                side: l.side,
                weight_bps: l.weight_bps,
            })
            .collect::<Vec<_>>();
        basket::validate_template(&specs, 2, state.config.max_basket_legs)
            .map_err(|e| AppError::bad_request("INVALID_TEMPLATE", e.to_string()))?;
        queries::baskets::set_legs(
            &mut client,
            id,
            &specs
                .iter()
                .map(|s| (s.market_id.clone(), s.side, s.weight_bps))
                .collect::<Vec<_>>(),
        )
        .await?;
    }

    queries::baskets::update_template(
        &client,
        id,
        req.title.as_deref(),
        req.description.as_deref(),
        req.category.as_deref(),
        req.cover_url.as_deref(),
        min_buy,
        max_buy,
    )
    .await?;

    let view = admin_view(&state, &client, id).await?;
    Ok(Json(view))
}

/// `POST /v1/admin/baskets/{id}/publish` — go live after a final validation
/// pass that checks every leg is tradable with sensible close times.
pub async fn publish(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<AdminBasketView>, AppError> {
    let id = parse_id(&id)?;
    let client = state.db.get().await?;

    let template = queries::baskets::get_template(&client, id)
        .await?
        .ok_or_else(|| AppError::not_found("basket template not found"))?;
    if template.status != "draft" {
        return Err(AppError::bad_request(
            "TEMPLATE_NOT_DRAFT",
            "only draft templates can be published",
        ));
    }

    // Publish-time liveness rules (brief §7.2): every leg tradable and
    // closing with at least 24 h of lead time. Warnings (not blockers) are
    // surfaced in the view for thin markets.
    let legs = queries::baskets::get_legs(&client, id).await?;
    if legs.iter().any(|l| !l.tradable()) {
        return Err(AppError::bad_request(
            "LEG_NOT_TRADABLE",
            "all legs must sit on open, primary-phase markets to publish",
        ));
    }
    let soonest = legs
        .iter()
        .filter_map(|l| l.close_time)
        .min()
        .ok_or_else(|| AppError::bad_request("LEG_NOT_TRADABLE", "legs have no close time"))?;
    if soonest < chrono::Utc::now() + chrono::Duration::hours(24) {
        return Err(AppError::bad_request(
            "CLOSE_TIME_TOO_SOON",
            "every leg must close at least 24 hours from now",
        ));
    }

    queries::baskets::set_template_status(&client, id, "published").await?;
    let view = admin_view(&state, &client, id).await?;
    Ok(Json(view))
}

/// `POST /v1/admin/baskets/{id}/close` — stop new purchases (holders keep
/// their positions; resolution is unaffected).
pub async fn close(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<AdminBasketView>, AppError> {
    let id = parse_id(&id)?;
    let client = state.db.get().await?;
    let template = queries::baskets::get_template(&client, id)
        .await?
        .ok_or_else(|| AppError::not_found("basket template not found"))?;
    if !matches!(template.status.as_str(), "draft" | "published") {
        return Err(AppError::bad_request(
            "TEMPLATE_NOT_OPEN",
            format!("a {} template cannot be closed", template.status),
        ));
    }
    queries::baskets::set_template_status(&client, id, "closed").await?;
    let view = admin_view(&state, &client, id).await?;
    Ok(Json(view))
}

// --- helpers -----------------------------------------------------------------

fn parse_id(path: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(path).map_err(|_| AppError::bad_request("INVALID_ID", "id must be a uuid"))
}

/// Money strings in admin payloads are base-unit integer strings.
fn parse_money(field: &str, s: &str) -> Result<i64, AppError> {
    s.parse().map_err(|_| {
        AppError::bad_request(
            "INVALID_TEMPLATE",
            format!("{field} must be base-unit integer string"),
        )
    })
}

/// Assemble the admin view with validation warnings.
async fn admin_view(
    _state: &AppState,
    client: &deadpool_postgres::Client,
    id: Uuid,
) -> Result<AdminBasketView, AppError> {
    let t = queries::baskets::get_template(client, id)
        .await?
        .ok_or_else(|| AppError::not_found("basket template not found"))?;
    let legs = queries::baskets::get_legs(client, id).await?;

    // Curation warnings (brief §7.9): anything an admin should know before
    // publishing — thin volume, close time approaching, untradable legs.
    let mut warnings = Vec::new();
    for leg in &legs {
        if !leg.tradable() {
            warnings.push(format!(
                "{} is not tradable (missing or not primary)",
                leg.market_id
            ));
        }
        if let Some(close) = leg.close_time {
            if close < chrono::Utc::now() + chrono::Duration::hours(48) {
                warnings.push(format!("{} closes in less than 48 hours", leg.market_id));
            }
        }
    }

    Ok(AdminBasketView {
        id: t.id,
        slug: t.slug,
        title: t.title,
        description: t.description,
        category: t.category,
        cover_url: t.cover_url,
        status: parse_status(&t.status),
        min_buy_usdc: t.min_buy_usdc.to_string(),
        max_buy_usdc: t.max_buy_usdc.to_string(),
        legs: legs
            .iter()
            .map(|l| crate::domain::AdminBasketLeg {
                market_id: l.market_id.clone(),
                side: if l.side == "yes" {
                    crate::domain::Side::Yes
                } else {
                    crate::domain::Side::No
                },
                weight_bps: l.weight_bps,
            })
            .collect(),
        warnings,
        created_at: t.created_at,
        published_at: t.published_at,
        closed_at: t.closed_at,
    })
}

fn parse_status(s: &str) -> crate::domain::BasketStatus {
    match s {
        "published" => crate::domain::BasketStatus::Published,
        "closed" => crate::domain::BasketStatus::Closed,
        "settled" => crate::domain::BasketStatus::Settled,
        _ => crate::domain::BasketStatus::Draft,
    }
}
