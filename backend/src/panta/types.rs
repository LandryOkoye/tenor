//! Wire types: exact JSON shapes documented at https://docs.panta.market
//! (recorded in docs/panta-notes.md — never invent fields).
//!
//! Rules used throughout:
//! - Field names are renamed to Panta's camelCase (`#[serde(rename = "…")]`).
//! - Anything Panta may omit is `Option` with `#[serde(default)]`, and unknown
//!   fields are tolerated (serde's default), so Panta can add fields safely.
//! - Decimal amounts/prices arrive as human-readable strings and are converted
//!   to base units only by the calling code (helpers in `panta/mod.rs`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A string that Panta sometimes sends as a JSON string and sometimes as a
/// number (e.g. trade ids, amounts). Normalized to `String` on deserialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrNum {
    S(String),
    N(f64),
}

impl StringOrNum {
    pub fn as_str(&self) -> String {
        match self {
            StringOrNum::S(s) => s.clone(),
            StringOrNum::N(n) => {
                // Print integral numbers without a trailing ".0" for cleaner display.
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Markets catalog
// ---------------------------------------------------------------------------

/// Filter for `GET /markets/`.
#[derive(Debug, Clone, Default)]
pub struct MarketFilter {
    pub category: Option<String>,
    /// `primary` | `secondary` | `resolved` | `cancelled`.
    pub status: Option<String>,
    /// Opaque cursor from a previous page's `nextCursor`.
    pub cursor: Option<String>,
    /// Page size, max 50.
    pub limit: Option<u32>,
}

/// One market row from `GET /markets/` or `GET /markets/{marketId}/`.
/// List rows leave `yes_price` / `no_price` null; the detail endpoint fills them.
/// Market times arrive as unix seconds on live rows but as RFC 3339
/// strings on sandbox (`pk_test_`) rows; accept both, store unix seconds.
fn unix_or_iso<'de, D>(d: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Ts {
        Unix(i64),
        Iso(String),
    }
    match Option::<Ts>::deserialize(d)? {
        None => Ok(None),
        Some(Ts::Unix(n)) => Ok(Some(n)),
        Some(Ts::Iso(s)) => DateTime::parse_from_rfc3339(&s)
            .map(|t| Some(t.timestamp()))
            .map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRow {
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(default)]
    pub category: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default, rename = "marketType")]
    pub market_type: Option<String>,
    #[serde(default, rename = "startTime", deserialize_with = "unix_or_iso")]
    pub start_time: Option<i64>, // unix seconds
    #[serde(default, rename = "endTime", deserialize_with = "unix_or_iso")]
    pub end_time: Option<i64>, // unix seconds
    #[serde(default, rename = "resolutionTime", deserialize_with = "unix_or_iso")]
    pub resolution_time: Option<i64>, // unix seconds
    #[serde(default)]
    pub region: Option<String>,
    /// Wallet that created the market (`creatorAddress`; detail rows).
    #[serde(
        default,
        rename = "creatorAddress",
        skip_serializing_if = "Option::is_none"
    )]
    pub creator_address: Option<String>,
    /// When the market actually resolved, if Panta reports it (unix or ISO).
    #[serde(
        default,
        rename = "resolvedAt",
        deserialize_with = "unix_or_iso",
        skip_serializing_if = "Option::is_none"
    )]
    pub resolved_at: Option<i64>,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub status: Option<String>,
    /// Human-readable volume string, e.g. "1200.00".
    #[serde(default, rename = "volumeUsdc")]
    pub volume_usdc: Option<String>,
    #[serde(default, rename = "createdByPartner")]
    pub created_by_partner: bool,
    /// Decimal price strings 0..=1; null on list rows, filled on detail.
    #[serde(default, rename = "yesPrice")]
    pub yes_price: Option<String>,
    #[serde(default, rename = "noPrice")]
    pub no_price: Option<String>,
    #[serde(default, rename = "primaryYesPrice")]
    pub primary_yes_price: Option<String>,
    #[serde(default, rename = "primaryNoPrice")]
    pub primary_no_price: Option<String>,
    #[serde(default, rename = "secondaryYesPrice")]
    pub secondary_yes_price: Option<String>,
    #[serde(default, rename = "secondaryNoPrice")]
    pub secondary_no_price: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMarketsResponse {
    pub items: Vec<MarketRow>,
    #[serde(default, rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------
// Solana instructions (returned by build endpoints)
// ---------------------------------------------------------------------------

/// One account reference inside a Panta instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireAccount {
    pub pubkey: String,
    #[serde(default, rename = "isSigner")]
    pub is_signer: bool,
    #[serde(default, rename = "isWritable")]
    pub is_writable: bool,
}

/// A Solana instruction exactly as Panta serializes it
/// (`{ programId, data: base64, accounts: [...] }`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireInstruction {
    #[serde(rename = "programId")]
    pub program_id: String,
    /// Base64-encoded instruction data.
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub accounts: Vec<WireAccount>,
}

// ---------------------------------------------------------------------------
// Primary buy flow
// ---------------------------------------------------------------------------

/// `POST /primaryorderquote/` — amountUsdc is human-readable ("20.00").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub wallet: String,
    #[serde(rename = "marketId")]
    pub market_id: String,
    /// "yes" or "no" (Panta is case-insensitive; we always send lowercase).
    pub side: String,
    #[serde(rename = "amountUsdc")]
    pub amount_usdc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteResponse {
    #[serde(rename = "quoteId")]
    pub quote_id: String,
    #[serde(rename = "marketId")]
    pub market_id: String,
    pub side: String,
    #[serde(rename = "amountUsdc")]
    pub amount_usdc: String,
    /// Human-readable share count, e.g. "38.42".
    pub shares: String,
    /// Average fill price, e.g. "0.520800". The sandbox (`pk_test_`) omits
    /// it — use `avg_price()` which derives it from amount / shares.
    #[serde(default, rename = "avgPrice")]
    pub avg_price: String,
    /// Protocol fee, human-readable USDC, e.g. "0.40".
    #[serde(rename = "feeUsdc")]
    pub fee_usdc: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,
    #[serde(default, rename = "blockhashExpiryHintSec")]
    pub blockhash_expiry_hint_sec: Option<u64>,
}

impl QuoteResponse {
    /// Average fill price: Panta's value when present, else amount / shares
    /// (6 decimals, integer math). Empty if neither can be computed.
    pub fn avg_price(&self) -> String {
        if !self.avg_price.is_empty() {
            return self.avg_price.clone();
        }
        let (Ok(amount), Ok(shares)) = (
            super::usdc_human_to_base(&self.amount_usdc),
            super::usdc_human_to_base(&self.shares),
        ) else {
            return String::new();
        };
        if shares <= 0 {
            return String::new();
        }
        let price = amount as i128 * 1_000_000 / shares as i128;
        format!("{}.{:06}", price / 1_000_000, price % 1_000_000)
    }
}

/// `POST /primaryorderbuild/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRequest {
    #[serde(rename = "quoteId")]
    pub quote_id: String,
    pub wallet: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(
        default,
        rename = "maxSlippageBps",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_slippage_bps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResponse {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "quoteId")]
    pub quote_id: String,
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default, rename = "marketId")]
    pub market_id: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default, rename = "amountUsdc")]
    pub amount_usdc: Option<String>,
    #[serde(default, rename = "expectedShares")]
    pub expected_shares: Option<String>,
    #[serde(default, rename = "feeUsdc")]
    pub fee_usdc: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub instructions: Vec<WireInstruction>,
    /// Extra derived accounts Panta may include (kept raw; not used by us).
    #[serde(default)]
    pub derived: Option<serde_json::Value>,
    #[serde(default, rename = "recentBlockhash")]
    pub recent_blockhash: Option<String>,
    #[serde(default, rename = "lastValidBlockHeight")]
    pub last_valid_block_height: Option<u64>,
    #[serde(default, rename = "expiresAt")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default, rename = "blockhashExpiryHintSec")]
    pub blockhash_expiry_hint_sec: Option<u64>,
}

/// `POST /primaryordersubmit/` — idempotent on (orderId, signature).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitRequest {
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitResponse {
    #[serde(default, rename = "orderId")]
    pub order_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
}

/// `POST /primaryorderverify/` body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(default)]
    pub status: Option<String>, // built | submitted | confirmed | failed | expired
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default, rename = "marketId")]
    pub market_id: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
}

// ---------------------------------------------------------------------------
// Positions & claims
// ---------------------------------------------------------------------------

/// One holding row from `GET /positions/` (one row per market+side).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionRow {
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(default)]
    pub category: Option<String>,
    pub side: String, // "yes" | "no"
    /// Human-readable share count, e.g. "38.40".
    pub shares: String,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub claimable: bool,
    #[serde(default)]
    pub claimed: Option<bool>,
    /// "yes"/"no" after resolution; null before.
    #[serde(default)]
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionsResponse {
    pub wallet: String,
    #[serde(default)]
    pub positions: Vec<PositionRow>,
}

/// `POST /claim/build/` body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimBuildRequest {
    pub wallet: String,
    #[serde(rename = "marketId")]
    pub market_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimBuildResponse {
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default, rename = "marketId")]
    pub market_id: Option<String>,
    /// Winning outcome label as Panta prints it, e.g. "YES".
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default, rename = "winningShares")]
    pub winning_shares: Option<String>,
    #[serde(default)]
    pub instructions: Vec<WireInstruction>,
    #[serde(default)]
    pub derived: Option<serde_json::Value>,
    #[serde(default, rename = "recentBlockhash")]
    pub recent_blockhash: Option<String>,
    #[serde(default, rename = "lastValidBlockHeight")]
    pub last_valid_block_height: Option<u64>,
}

// ---------------------------------------------------------------------------
// Attribution (trade reporting)
// ---------------------------------------------------------------------------

/// `POST /trades/` — explicit attribution report (idempotent per signature).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportTradeRequest {
    pub signature: String,
    pub wallet: String,
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<String>,
    #[serde(
        default,
        rename = "clientOrderId",
        skip_serializing_if = "Option::is_none"
    )]
    pub client_order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportTradeResponse {
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub status: Option<String>, // "processed"
    #[serde(default, rename = "marketId")]
    pub market_id: Option<String>,
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default)]
    pub side: Option<String>,
    #[serde(default)]
    pub kind: Option<String>, // "buy" | "claim"
}

/// `GET /trades/{signature}/` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeStatusResponse {
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub status: Option<String>, // processed | pending_attribution | unknown | failed
    #[serde(default, rename = "marketId")]
    pub market_id: Option<String>,
    #[serde(default)]
    pub wallet: Option<String>,
}

// ---------------------------------------------------------------------------
// Trade tapes
// ---------------------------------------------------------------------------

/// One row of a market or wallet trade tape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRow {
    #[serde(default)]
    pub id: Option<StringOrNum>,
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(default)]
    pub wallet: String,
    #[serde(default, rename = "isPrimary")]
    pub is_primary: bool,
    #[serde(default, rename = "yesAmount")]
    pub yes_amount: Option<StringOrNum>,
    #[serde(default, rename = "noAmount")]
    pub no_amount: Option<StringOrNum>,
    #[serde(default, rename = "feePaid")]
    pub fee_paid: Option<StringOrNum>,
    #[serde(default, rename = "blockTime")]
    pub block_time: Option<i64>, // unix seconds
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default, rename = "quoteAsset")]
    pub quote_asset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketTradesResponse {
    #[serde(default, rename = "marketId")]
    pub market_id: Option<String>,
    #[serde(default)]
    pub items: Vec<TradeRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletTradesResponse {
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default)]
    pub items: Vec<TradeRow>,
}

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoriesResponse {
    pub categories: Vec<String>,
}

#[cfg(test)]
mod quote_tests {
    use super::*;

    #[test]
    fn sandbox_quote_without_avg_price_decodes() {
        // Verbatim shape returned by Panta for a pk_test_ key (11 Oct 2026).
        let q: QuoteResponse = serde_json::from_str(
            r#"{"quoteId":"qt_sandbox_test","marketId":"TestMarket1111111111111111111111111111111",
                "wallet":"W","side":"yes","amountUsdc":"1.00","shares":"2.00","feeUsdc":"0.00",
                "expiresAt":"2099-01-01T00:00:00Z","disclaimer":"Test mode"}"#,
        )
        .unwrap();
        assert_eq!(q.avg_price(), "0.500000");
    }

    #[test]
    fn market_times_accept_unix_and_iso() {
        let live: MarketRow =
            serde_json::from_str(r#"{"marketId":"m","title":"t","endTime":1798761599}"#).unwrap();
        let sandbox: MarketRow = serde_json::from_str(
            r#"{"marketId":"m","title":"t","endTime":"2026-12-31T23:59:59Z"}"#,
        )
        .unwrap();
        assert_eq!(live.end_time, Some(1_798_761_599));
        assert_eq!(sandbox.end_time, Some(1_798_761_599));
    }
}
