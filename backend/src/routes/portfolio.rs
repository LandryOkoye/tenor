//! Portfolio endpoints (brief §7.6, §9): positions, baskets, P&L, activity.
//!
//! Data sources are merged honestly:
//! - WHAT the wallet holds comes from Panta positions (the chain is truth)
//! - WHAT it cost comes from our orders
//! - WHAT it's worth comes from our cached market prices
//! If the wallet's shares no longer match what we recorded, the basket is
//! flagged "modified outside APP" (brief §7.6).

use axum::{extract::State, Json};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::{
    ActivityItem, ActivityKind, Portfolio, PortfolioBasket, PortfolioBasketLeg,
    PortfolioPosition, Side,
};
use crate::error::AppError;
use crate::panta::{self, PantaApi};
use crate::queries;
use crate::state::AppState;

/// `GET /v1/portfolio`
pub async fn portfolio(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Portfolio>, AppError> {
    let client = state.db.get().await?;

    // 1. Chain truth: what the wallet actually holds (shares, claimable).
    let chain_positions = state.panta.positions(&user.wallet).await?;

    // 2. Our truth: every order we recorded for this user.
    let orders = queries::portfolio::orders_for_user(&client, user.user_id).await?;

    // Map for on-chain share lookup: (market_id, side) -> shares in base units.
    let chain_shares = |market_id: &str, side: &str| -> Option<i64> {
        chain_positions
            .iter()
            .find(|p| p.market_id == market_id && p.side == side)
            .and_then(|p| p.usdc_human_to_base_shares())
    };

    // 3. Valuation helper per Panta's documented rule set (docs/panta-notes):
    //    open -> shares × side price; resolved win -> shares × 1; loss -> 0.
    let value_of = |shares_base: i64,
                    order: &queries::portfolio::PortfolioOrder|
     -> (String, Option<String>) {
        let side_price = if order.side == "yes" {
            &order.yes_price
        } else {
            &order.no_price
        };
        if order.resolved {
            let won = order.outcome.as_deref() == Some(order.side.as_str());
            let value = if won { shares_base } else { 0 };
            (value.to_string(), side_price.clone())
        } else {
            let price_base = side_price
                .as_deref()
                .and_then(|p| panta::usdc_human_to_base(p).ok())
                .unwrap_or(1_000_000);
            (shares_base * price_base / 1_000_000, side_price.clone())
        }
    };

    // 4. Basket groupings: orders linked to a purchase, keyed by purchase.
    //    (The smoke flow and the basket endpoints create these links.)
    let mut basket_orders: std::collections::HashMap<Uuid, Vec<&queries::portfolio::PortfolioOrder>> =
        std::collections::HashMap::new();
    for o in &orders {
        if let Some(pid) = o.basket_purchase_id {
            basket_orders.entry(pid).or_default().push(o);
        }
    }

    let mut baskets = Vec::with_capacity(basket_orders.len());
    let mut total_value: i64 = 0;
    let mut total_cost: i64 = 0;

    for (purchase_id, legs) in basket_orders {
        let mut leg_views = Vec::with_capacity(legs.len());
        let mut cost: i64 = 0;
        let mut value: i64 = 0;
        let mut resolved_count = 0;
        let mut modified_outside = false;

        for leg in &legs {
            cost += leg.amount_usdc;
            let on_chain = chain_shares(&leg.market_id, &leg.side);
            let recorded = leg.expected_shares.unwrap_or(0);
            // Modified outside APP: the chain has fewer shares than we recorded.
            if let Some(actual) = on_chain {
                if actual < recorded {
                    modified_outside = true;
                }
            }
            let (leg_value, price) = value_of(on_chain.unwrap_or(recorded), leg);
            let leg_value_i = leg_value.parse::<i64>().unwrap_or(0);
            value += leg_value_i;
            if leg.status == "resolved_win" || leg.status == "resolved_loss" || leg.status == "claimed" {
                resolved_count += 1;
            }
            leg_views.push(PortfolioBasketLeg {
                order_id: leg.id,
                market_id: leg.market_id.clone(),
                market_title: leg.market_title.clone().unwrap_or_else(|| leg.market_id.clone()),
                side: if leg.side == "yes" { Side::Yes } else { Side::No },
                shares: on_chain.map(|s| s.to_string()),
                status: parse_leg_status(&leg.status),
                value_usdc: leg_value_i.to_string(),
                claimable: chain_positions
                    .iter()
                    .any(|p| p.market_id == leg.market_id && p.side == leg.side && p.claimable),
            });
        }

        // Purchase-level info (slug/title for display).
        let purchase = queries::baskets::get_purchase(&client, purchase_id).await?;
        let (slug, title) = match &purchase {
            Some(p) => (p.template_slug.clone(), p.template_title.clone()),
            None => (None, None),
        };

        total_value += value;
        total_cost += cost;
        baskets.push(PortfolioBasket {
            purchase_id,
            template_slug: slug.unwrap_or_default(),
            template_title: title.unwrap_or_else(|| "Basket".to_string()),
            status: parse_purchase_status(&purchase.map(|p| p.status).unwrap_or_default()),
            cost_basis_usdc: cost.to_string(),
            current_value_usdc: value.to_string(),
            unrealized_pnl_usdc: (value - cost).to_string(),
            legs_resolved: resolved_count,
            legs_total: legs.len() as i32,
            legs: leg_views,
            modified_outside,
        });
    }

    // 5. Single positions: on-chain holdings with no linked basket order.
    //    (A user may hold shares bought elsewhere — we still show them.)
    let basket_legs: std::collections::HashSet<(String, String)> = orders
        .iter()
        .filter_map(|o| {
            o.basket_purchase_id
                .map(|_| (o.market_id.clone(), o.side.clone()))
        })
        .collect();
    let mut single_positions = Vec::new();
    for pos in &chain_positions {
        if basket_legs.contains(&(pos.market_id.clone(), pos.side.clone())) {
            continue; // counted inside a basket above
        }
        let shares = pos.usdc_human_to_base_shares().unwrap_or(0);
        let market_row = queries::markets::get_market(&client, &pos.market_id).await?;
        let (price, resolved, outcome, title) = match market_row {
            Some(m) => (
                if pos.side == "yes" { m.yes_price } else { m.no_price },
                m.resolved,
                m.outcome,
                Some(m.title),
            ),
            None => (None, false, None, None),
        };
        let pseudo = queries::portfolio::PortfolioOrder {
            id: Uuid::nil(),
            basket_purchase_id: None,
            market_id: pos.market_id.clone(),
            market_title: title,
            side: pos.side.clone(),
            amount_usdc: 0,
            expected_shares: Some(shares),
            status: "filled".into(),
            tx_signature: None,
            created_at: chrono::Utc::now(),
            yes_price: price.clone(),
            no_price: price.clone(),
            phase: pos.phase.clone(),
            resolved,
            outcome,
        };
        let (value, current_price) = value_of(shares, &pseudo);
        let value_i = value.parse::<i64>().unwrap_or(0);
        total_value += value_i;
        single_positions.push(PortfolioPosition {
            market_id: pos.market_id.clone(),
            market_title: pseudo.market_title.clone().unwrap_or_else(|| pos.market_id.clone()),
            side: if pos.side == "yes" { Side::Yes } else { Side::No },
            shares: shares.to_string(),
            current_price,
            value_usdc: value_i.to_string(),
            cost_basis_usdc: None, // not one of ours; cost is unknown
            unrealized_pnl_usdc: None,
            resolved: pos.outcome.is_some(),
            won: pos.outcome.as_ref().map(|o| o == &pos.side),
            claimable: pos.claimable,
        });
    }

    Ok(Json(Portfolio {
        wallet: user.wallet,
        single_positions,
        baskets,
        total_value_usdc: total_value.to_string(),
        total_cost_basis_usdc: total_cost.to_string(),
        total_unrealized_pnl_usdc: (total_value - total_cost).to_string(),
    }))
}

/// `GET /v1/portfolio/activity` — orders + claims, newest first.
pub async fn activity(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<ActivityItem>>, AppError> {
    let client = state.db.get().await?;
    let rows = queries::portfolio::activity(&client, user.user_id, 50).await?;

    Ok(Json(
        rows.into_iter()
            .map(|r| ActivityItem {
                kind: if r.kind == "claim" { ActivityKind::Claim } else if r.basket_purchase_id.is_some() { ActivityKind::BasketBuy } else { ActivityKind::Buy },
                market_id: Some(r.market_id),
                market_title: r.market_title,
                basket_purchase_id: r.basket_purchase_id,
                side: r.side.map(|s| if s == "yes" { Side::Yes } else { Side::No }),
                amount_usdc: r.amount_usdc.to_string(),
                tx_signature: r.tx_signature,
                ts: r.created_at,
            })
            .collect(),
    ))
}

// --- helpers -----------------------------------------------------------------

fn parse_leg_status(s: &str) -> crate::domain::LegStatus {
    match s {
        "filled" => crate::domain::LegStatus::Filled,
        "failed" => crate::domain::LegStatus::Failed,
        "resolved_win" => crate::domain::LegStatus::ResolvedWin,
        "resolved_loss" => crate::domain::LegStatus::ResolvedLoss,
        "claimed" => crate::domain::LegStatus::Claimed,
        _ => crate::domain::LegStatus::Pending,
    }
}

fn parse_purchase_status(s: &str) -> crate::domain::PurchaseStatus {
    match s {
        "built" => crate::domain::PurchaseStatus::Built,
        "submitted" => crate::domain::PurchaseStatus::Submitted,
        "confirmed" => crate::domain::PurchaseStatus::Confirmed,
        "filled" => crate::domain::PurchaseStatus::Filled,
        "partial" => crate::domain::PurchaseStatus::Partial,
        "failed" => crate::domain::PurchaseStatus::Failed,
        "active" => crate::domain::PurchaseStatus::Active,
        "partially_resolved" => crate::domain::PurchaseStatus::PartiallyResolved,
        "resolved" => crate::domain::PurchaseStatus::Resolved,
        "claimed" => crate::domain::PurchaseStatus::Claimed,
        _ => crate::domain::PurchaseStatus::Quoted,
    }
}

// Small extension: read a PositionRow's shares as base units.
trait SharesInBase {
    fn usdc_human_to_base_shares(&self) -> Option<i64>;
}
impl SharesInBase for crate::panta::PositionRow {
    fn usdc_human_to_base_shares(&self) -> Option<i64> {
        panta::usdc_human_to_base(&self.shares).ok()
    }
}
