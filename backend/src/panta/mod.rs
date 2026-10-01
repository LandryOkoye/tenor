//! Typed client for the Panta API (https://docs.panta.market).
//!
//! Everything Panta-related flows through the `PantaApi` trait in `client.rs`:
//! - `http.rs` — the real implementation over HTTPS (X-Api-Key auth, retries,
//!   per-endpoint rate limiting).
//! - `mock.rs` — a deterministic in-memory implementation for tests and
//!   offline development (enabled with `PANTA_MOCK=true`).
//!
//! Wire types mirror the *documented* Panta JSON exactly (see `types.rs` and
//! `docs/panta-notes.md` — never invent fields).
//!
//! Money convention: Panta speaks human-readable decimal strings ("20.00");
//! we speak integer base units (i64, 6 decimals) everywhere else. The two
//! helpers at the bottom of this file are the only place that conversion
//! happens, and they use pure string math — never floats (brief §6.3).

mod client;
mod error;
mod http;
mod mock;
mod rate;
mod types;

pub use client::{make_client, PantaApi};
pub use error::PantaError;
pub use mock::MockPanta;
pub use types::*;

/// Convert USDC base units -> Panta's human-readable decimal string.
///
/// `20000000` -> `"20"`;  `500000` -> `"0.5"`;  `150` -> `"0.00015"`.
/// Trailing zeros are trimmed; a zero amount becomes `"0"`.
pub fn usdc_base_to_human(base: i64) -> String {
    let sign = if base < 0 { "-" } else { "" };
    let base = base.unsigned_abs();

    // Split into whole and 6-digit fractional parts.
    let whole = base / 1_000_000;
    let mut frac = format!("{:06}", base % 1_000_000);

    // Trim trailing zeros so "20.000000" becomes "20", "0.500000" -> "0.5".
    while frac.ends_with('0') {
        frac.pop();
    }

    if frac.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{frac}")
    }
}

/// Convert Panta's human-readable decimal string -> USDC base units.
///
/// `"20.00"` -> `20000000`;  `"0.5"` -> `500000`.
/// Fails on malformed input or more than 6 decimal places (never rounds
/// silently — money must be exact).
pub fn usdc_human_to_base(human: &str) -> Result<i64, String> {
    let human = human.trim();
    let (sign, digits) = match human.as_bytes().first() {
        Some(b'-') => (-1i64, &human[1..]),
        Some(b'+') => (1, &human[1..]),
        _ => (1, human),
    };

    // Split on the decimal point (at most one).
    let (whole, frac) = match digits.split_once('.') {
        Some((w, f)) => (w, f),
        None => (digits, ""),
    };

    if whole.is_empty() && frac.is_empty() {
        return Err(format!("not a number: {human:?}"));
    }
    // Reject anything that is not plain digits (no exponents, no thousands separators).
    if !whole.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("not a plain decimal number: {human:?}"));
    }
    if frac.len() > 6 {
        return Err(format!("more than 6 decimal places: {human:?}"));
    }

    // "20" -> 20_000_000;  ".5" -> 500_000 (frac padded to exactly 6 digits).
    let whole_val: i64 = if whole.is_empty() {
        0
    } else {
        whole
            .parse()
            .map_err(|_| format!("amount too large: {human:?}"))?
    };
    let frac_val: i64 = if frac.is_empty() {
        0
    } else {
        let padded = format!("{frac:0<6}"); // "5" -> "500000"
        padded
            .parse()
            .map_err(|_| format!("amount too large: {human:?}"))?
    };

    let base = whole_val
        .checked_mul(1_000_000)
        .and_then(|v| v.checked_add(frac_val))
        .ok_or_else(|| format!("amount too large: {human:?}"))?;
    Ok(sign * base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_to_human_round_trips() {
        assert_eq!(usdc_base_to_human(20_000_000), "20");
        assert_eq!(usdc_base_to_human(500_000), "0.5");
        assert_eq!(usdc_base_to_human(150), "0.00015");
        assert_eq!(usdc_base_to_human(0), "0");
        assert_eq!(usdc_base_to_human(1), "0.000001");

        // Every base value we produce must parse back exactly.
        for base in [0, 1, 150, 999_999, 1_000_000, 20_000_000, i64::MAX] {
            let human = usdc_base_to_human(base);
            assert_eq!(usdc_human_to_base(&human).unwrap(), base, "base {base}");
        }
    }

    #[test]
    fn human_to_base_rejects_garbage() {
        assert!(usdc_human_to_base("12.3456789").is_err()); // 7 decimals
        assert!(usdc_human_to_base("abc").is_err());
        assert!(usdc_human_to_base("1.2.3").is_err());
        assert!(usdc_human_to_base("1e5").is_err());
        assert!(usdc_human_to_base("").is_err());
        assert_eq!(usdc_human_to_base("20.00").unwrap(), 20_000_000);
        assert_eq!(usdc_human_to_base("0.000001").unwrap(), 1);
    }
}
