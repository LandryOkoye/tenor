//! Portfolio endpoints (brief §7.6, §9): positions, baskets, P&L, activity.
//!
//! Data sources are merged honestly:
//!   - WHAT the wallet holds comes from Panta positions (the chain is truth)
//!   - WHAT it cost comes from our orders
//!   - WHAT it's worth comes from our cached market prices
//!
//! If the wallet's shares no longer match what we recorded, the basket is
//! flagged "modified outside Tenor" (brief §7.6).

use axum::{extract::State, Json};
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::{
    ActivityItem, ActivityKind, Portfolio, PortfolioBasket, PortfolioBasketLeg, PortfolioPosition,
    Side,
};
use crate::error::AppError;
use crate::panta;
use crate::queries;
use crate::state::AppState;

/// Order statuses that mean "the user actually holds (or held) these shares".
/// Quoted/built/submitted/failed orders never touched the chain.
const HELD_STATUSES: [&str; 4] = ["filled", "resolved_win", "resolved_loss", "claimed"];

/// `GET /v1/portfolio`
///
/// Share attribution: the wallet's on-chain shares per (market, side) are
/// handed out to our basket legs first (up to what each leg recorded); any
/// shares left over are shown as a single position — with a cost basis when
/// they came from single-market orders placed through the app. Nothing is
/// counted twice, even when two baskets hold the same market and side.
pub async fn portfolio(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Portfolio>, AppError> {
    // 1. Chain truth: what the wallet actually holds (shares, claimable).
    let chain_positions = state.panta.positions(&user.wallet).await?;
    let client = state.db.get().await?;

    // 2. Our truth: orders that really filled.
    let orders: Vec<queries::portfolio::PortfolioOrder> =
        queries::portfolio::orders_for_user(&client, user.user_id)
            .await?
            .into_iter()
            .filter(|o| HELD_STATUSES.contains(&o.status.as_str()))
            .collect();

    // Unattributed on-chain shares per (market, side), consumed as we go.
    let mut unattributed: HashMap<(String, String), i64> = HashMap::new();
    for p in &chain_positions {
        if let Some(shares) = p.usdc_human_to_base_shares() {
            *unattributed
                .entry((p.market_id.clone(), p.side.clone()))
                .or_default() += shares;
        }
    }
    let claimable = |market_id: &str, side: &str| {
        chain_positions
            .iter()
            .any(|p| p.market_id == market_id && p.side == side && p.claimable)
    };

    // 3. Baskets: legs grouped by purchase (oldest purchase first, so the
    //    attribution is stable between calls).
    let mut basket_orders: HashMap<Uuid, Vec<&queries::portfolio::PortfolioOrder>> = HashMap::new();
    for o in orders.iter().rev() {
        if let Some(pid) = o.basket_purchase_id {
            basket_orders.entry(pid).or_default().push(o);
        }
    }
    let mut purchase_ids: Vec<Uuid> = basket_orders.keys().copied().collect();
    purchase_ids.sort_by_key(|pid| basket_orders[pid][0].created_at);

    let mut baskets = Vec::with_capacity(purchase_ids.len());
    let mut total_value: i64 = 0;
    let mut total_cost: i64 = 0;

    for purchase_id in purchase_ids {
        let legs = &basket_orders[&purchase_id];
        let mut leg_views = Vec::with_capacity(legs.len());
        let (mut cost, mut value, mut resolved_count) = (0i64, 0i64, 0i32);
        let mut modified_outside = false;

        for leg in legs {
            cost += leg.amount_usdc;
            let recorded = leg.expected_shares.unwrap_or(0);
            let key = (leg.market_id.clone(), leg.side.clone());

            // Claimed legs were paid out already; they hold nothing now.
            let shares = if leg.status == "claimed" {
                0
            } else {
                match unattributed.get_mut(&key) {
                    Some(available) => {
                        let take = recorded.min(*available);
                        if take < recorded {
                            modified_outside = true; // fewer shares than we bought
                        }
                        *available -= take;
                        take
                    }
                    // Not on chain (yet): the positions indexer lags fresh buys.
                    None => recorded,
                }
            };
            let leg_value = if leg.status == "claimed" {
                0
            } else {
                value_of(shares, leg.amount_usdc, leg)
            };
            value += leg_value;
            if matches!(
                leg.status.as_str(),
                "resolved_win" | "resolved_loss" | "claimed"
            ) {
                resolved_count += 1;
            }
            leg_views.push(PortfolioBasketLeg {
                order_id: leg.id,
                market_id: leg.market_id.clone(),
                market_title: leg
                    .market_title
                    .clone()
                    .unwrap_or_else(|| leg.market_id.clone()),
                side: side_of(&leg.side),
                shares: Some(shares.to_string()),
                status: parse_leg_status(&leg.status),
                value_usdc: leg_value.to_string(),
                claimable: claimable(&leg.market_id, &leg.side),
            });
        }

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

    // 4. Single positions: on-chain shares not attributed to a basket leg.
    //    Cost basis comes from the app's own single-market orders, if any.
    let mut single_cost: HashMap<(String, String), i64> = HashMap::new();
    for o in orders.iter().filter(|o| o.basket_purchase_id.is_none()) {
        *single_cost
            .entry((o.market_id.clone(), o.side.clone()))
            .or_default() += o.amount_usdc;
    }

    let mut single_positions = Vec::new();
    for pos in &chain_positions {
        let key = (pos.market_id.clone(), pos.side.clone());
        let shares = unattributed.remove(&key).unwrap_or(0);
        if shares <= 0 && !pos.claimable {
            continue; // fully covered by basket legs
        }
        let market = queries::markets::get_market(&client, &pos.market_id).await?;
        let cost = single_cost.get(&key).copied();
        let (price, resolved, outcome, title) = match market {
            Some(m) => (
                if pos.side == "yes" {
                    m.yes_price
                } else {
                    m.no_price
                },
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
            market_title: title.clone(),
            side: pos.side.clone(),
            amount_usdc: cost.unwrap_or(0),
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
        let value = value_of(shares, cost.unwrap_or(0), &pseudo);
        total_value += value;
        if let Some(c) = cost {
            total_cost += c;
        }
        single_positions.push(PortfolioPosition {
            market_id: pos.market_id.clone(),
            market_title: title.unwrap_or_else(|| pos.market_id.clone()),
            side: side_of(&pos.side),
            shares: shares.to_string(),
            current_price: price,
            value_usdc: value.to_string(),
            cost_basis_usdc: cost.map(|c| c.to_string()),
            unrealized_pnl_usdc: cost.map(|c| (value - c).to_string()),
            resolved: pos.outcome.is_some(),
            won: pos.outcome.as_ref().map(|o| o == &pos.side),
            claimable: pos.claimable,
        });
    }

    // Tokenized baskets: valued on chain; counted in totals when readable.
    drop(client);
    let vault_holdings = crate::routes::vault::holdings(&state, user.user_id, &user.wallet).await?;
    for h in &vault_holdings {
        if let Some(v) = h.value_usdc.as_deref().and_then(|v| v.parse::<i64>().ok()) {
            total_value += v;
            total_cost += h.cost_basis_usdc.parse::<i64>().unwrap_or(0);
        }
    }

    Ok(Json(Portfolio {
        wallet: user.wallet,
        single_positions,
        baskets,
        vault_holdings,
        total_value_usdc: total_value.to_string(),
        total_cost_basis_usdc: total_cost.to_string(),
        total_unrealized_pnl_usdc: (total_value - total_cost).to_string(),
    }))
}

/// Valuation per Panta's documented rules (docs/panta-notes.md):
/// resolved win -> 1 USDC/share, resolved loss -> 0, open -> shares × side
/// price. An open market with no price yet is carried at cost (neutral P&L)
/// rather than inventing a value.
fn value_of(shares_base: i64, cost_base: i64, order: &queries::portfolio::PortfolioOrder) -> i64 {
    if order.resolved {
        return match order.outcome.as_deref() {
            Some(o) if o == order.side => shares_base,
            Some(_) => 0,
            None => cost_base, // resolved flag without an outcome yet
        };
    }
    let side_price = if order.side == "yes" {
        &order.yes_price
    } else {
        &order.no_price
    };
    match side_price
        .as_deref()
        .and_then(|p| panta::usdc_human_to_base(p).ok())
    {
        Some(price_base) => (shares_base as i128 * price_base as i128 / 1_000_000) as i64,
        None => cost_base,
    }
}

fn side_of(s: &str) -> Side {
    if s == "yes" {
        Side::Yes
    } else {
        Side::No
    }
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
                kind: if r.kind == "claim" {
                    ActivityKind::Claim
                } else if r.basket_purchase_id.is_some() {
                    ActivityKind::BasketBuy
                } else {
                    ActivityKind::Buy
                },
                market_id: Some(r.market_id),
                market_title: r.market_title,
                basket_purchase_id: r.basket_purchase_id,
                side: r
                    .side
                    .map(|s| if s == "yes" { Side::Yes } else { Side::No }),
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
