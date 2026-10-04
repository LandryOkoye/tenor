//! Basket engine: allocation math, index pricing, and template validation
//! (brief §7). Pure functions — no I/O — so every rule is unit-testable.
//!
//! MVP frozen decisions (build plan §1): equal weights only, max 4 legs,
//! our fee 75 bps, min/max buy 1/100 USDC. The engine is written generically
//! (weights, fee bps) so changing those decisions later is a config change.

use crate::domain::{BasketLegView, Side};

/// Our fee on basket purchases, in basis points (config-driven at runtime;
/// tests use this default).
pub const DEFAULT_FEE_BPS: i64 = 75;

/// A basket template rule set (brief §7.2), checked before publishing and
/// again before quoting.
#[derive(Debug, thiserror::Error)]
pub enum BasketError {
    #[error("a basket needs between {min} and {max} legs, got {got}")]
    LegCount { min: usize, max: usize, got: usize },
    #[error("weights must be positive")]
    NonPositiveWeight,
    #[error("weights must sum to 10000 bps, got {got}")]
    WeightSum { got: i32 },
    #[error("market {0} appears more than once in one basket")]
    DuplicateMarket(String),
    #[error("a basket cannot hold both sides of market {0}")]
    BothSides(String),
    #[error("amount {amount} is outside the template's buy range ({min}..={max})")]
    AmountOutOfRange {
        amount: i64,
        min: i64,
        max: i64,
    },
}

/// One leg definition as used by the engine (side + weight on a market).
#[derive(Debug, Clone)]
pub struct LegSpec {
    pub market_id: String,
    pub side: Side,
    pub weight_bps: i32,
}

/// Validate template-level rules that need no database (brief §7.2).
/// Market liveness/liquidity checks happen at publish/quote time where we
/// can consult the catalog.
pub fn validate_template(legs: &[LegSpec], min_legs: usize, max_legs: usize) -> Result<(), BasketError> {
    if legs.len() < min_legs || legs.len() > max_legs {
        return Err(BasketError::LegCount {
            min: min_legs,
            max: max_legs,
            got: legs.len(),
        });
    }
    for leg in legs {
        if leg.weight_bps <= 0 {
            return Err(BasketError::NonPositiveWeight);
        }
    }
    let total: i32 = legs.iter().map(|l| l.weight_bps).sum();
    if total != 10_000 {
        return Err(BasketError::WeightSum { got: total });
    }
    for (i, a) in legs.iter().enumerate() {
        for b in legs.iter().skip(i + 1) {
            if a.market_id == b.market_id {
                if a.side != b.side {
                    return Err(BasketError::BothSides(a.market_id.clone()));
                }
                return Err(BasketError::DuplicateMarket(a.market_id.clone()));
            }
        }
    }
    Ok(())
}

/// Split a purchase amount into per-leg allocations (brief §7.3 step 2).
///
/// - our fee is taken off the top: `fee = amount * fee_bps / 10_000` (floor)
/// - the rest is split by weight, rounding DOWN on every leg
/// - the leftover dust is simply not spent — it stays in the user's wallet
///
/// Integer math only. Returns (allocations in leg order, fee, dust).
pub fn allocate(
    amount_base: i64,
    fee_bps: i64,
    weights_bps: &[i32],
) -> (Vec<i64>, i64, i64) {
    let fee = amount_base * fee_bps / 10_000;
    let net = amount_base - fee;

    let mut allocations = Vec::with_capacity(weights_bps.len());
    let mut spent: i64 = 0;
    for w in weights_bps {
        let alloc = net * *w as i64 / 10_000; // floor per leg
        allocations.push(alloc);
        spent += alloc;
    }
    let dust = net - spent;
    (allocations, fee, dust)
}

/// Basket "index price" (brief §9): the weighted average of the legs' current
/// side prices. Legs without a price are skipped and their weight excluded,
/// so the number is honest about what it covers (None when no legs price).
pub fn index_price(legs: &[BasketLegView]) -> Option<String> {
    let mut weight_sum: i64 = 0;
    let mut acc: i64 = 0;
    for leg in legs {
        if let Some(p) = &leg.current_price {
            if let Ok(base) = crate::panta::usdc_human_to_base(p) {
                acc += base * leg.weight_bps as i64;
                weight_sum += leg.weight_bps as i64;
            }
        }
    }
    if weight_sum == 0 {
        return None;
    }
    // Weighted average in 1e6 units; keep 4 decimals for display.
    let avg = acc / weight_sum;
    Some(format!("{:.4}", avg as f64 / 1_000_000.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(m: &str, side: Side, w: i32) -> LegSpec {
        LegSpec {
            market_id: m.into(),
            side,
            weight_bps: w,
        }
    }

    #[test]
    fn allocation_splits_by_weight_and_floors() {
        // 20 USDC, 75 bps fee -> fee 0.15, net 19.85.
        // Equal 2-leg split: 9.925 each, floor in base units is exact here.
        let (allocs, fee, dust) = allocate(20_000_000, 75, &[5_000, 5_000]);
        assert_eq!(fee, 150_000); // 0.15 USDC
        assert_eq!(allocs, vec![9_925_000, 9_925_000]);
        assert_eq!(dust, 0);
    }

    #[test]
    fn allocation_dust_stays_with_user() {
        // 1 USDC across 3 equal legs: net = 992_500; per leg 330_833.33 -> 330_833
        let (allocs, fee, dust) = allocate(1_000_000, 75, &[3_333, 3_333, 3_334]);
        assert_eq!(fee, 7_500);
        assert_eq!(allocs.len(), 3);
        assert!(allocs.iter().all(|a| *a <= 330_833));
        // Nothing lost: fee + allocated + dust == amount.
        assert_eq!(fee + allocs.iter().sum::<i64>() + dust, 1_000_000);
        assert!(dust >= 0);
    }

    #[test]
    fn weights_never_lose_money() {
        // Property: fee + sum(allocs) + dust always equals the input amount.
        for (amount, fee_bps) in [
            (1_000_000, 75),
            (3_500_000, 0),
            (123_456_789, 100),
            (7, 75),
        ] {
            let (allocs, fee, dust) = allocate(amount, fee_bps, &[2_500, 2_500, 5_000]);
            assert_eq!(fee + allocs.iter().sum::<i64>() + dust, amount);
        }
    }

    #[test]
    fn template_validation_rules() {
        let ok = vec![leg("a", Side::Yes, 5_000), leg("b", Side::No, 5_000)];
        assert!(validate_template(&ok, 2, 4).is_ok());

        // Too few / too many legs.
        assert!(validate_template(&ok, 3, 4).is_err());
        let five: Vec<_> = (0..5).map(|i| leg(&format!("m{i}"), Side::Yes, 2_000)).collect();
        assert!(validate_template(&five, 2, 4).is_err());

        // Bad weights.
        assert!(validate_template(&[leg("a", Side::Yes, 4_000), leg("b", Side::No, 5_000)], 2, 4).is_err());
        assert!(validate_template(&[leg("a", Side::Yes, 0), leg("b", Side::No, 10_000)], 2, 4).is_err());

        // Duplicate market and both-sides.
        assert!(validate_template(
            &[leg("a", Side::Yes, 5_000), leg("a", Side::Yes, 5_000)],
            2, 4
        )
        .is_err());
        assert!(validate_template(
            &[leg("a", Side::Yes, 5_000), leg("a", Side::No, 5_000)],
            2, 4
        )
        .is_err());
    }

    #[test]
    fn index_price_weights_legs() {
        let legs = vec![
            BasketLegView {
                market_id: "a".into(),
                market_title: "A".into(),
                side: Side::Yes,
                weight_bps: 4_000,
                current_price: Some("0.60".into()),
                close_time: None,
                tradable: true,
            },
            BasketLegView {
                market_id: "b".into(),
                market_title: "B".into(),
                side: Side::No,
                weight_bps: 6_000,
                current_price: Some("0.30".into()),
                close_time: None,
                tradable: true,
            },
        ];
        // 0.4*0.60 + 0.6*0.30 = 0.42
        assert_eq!(index_price(&legs).as_deref(), Some("0.4200"));

        // No priced legs -> None rather than a made-up number.
        let unpriced = vec![BasketLegView {
            current_price: None,
            ..legs.into_iter().next().unwrap()
        }];
        assert_eq!(index_price(&unpriced), None);
    }
}
