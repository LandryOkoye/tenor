//! Single-market order endpoints (brief §9): the quote → build → sign →
//! submit → confirm lifecycle for buying YES/NO on one market.
//!
//! Fees: single-market buys carry 0 bps of our fee in the MVP (frozen
//! decision in the build plan); the user only pays Panta's protocol fee.

use axum::{extract::{Path, State}, Json};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::{self, BuildResponse, OrderQuote, OrderQuoteRequest, OrderStatus, OrderView, Side, SubmitRequest, TxSummary};
use crate::error::AppError;
use crate::panta::{self, PantaApi};
use crate::queries;
use crate::solana_tx::{self, SolanaCtx, TxError};
use crate::state::AppState;

/// Sanity bounds on a single buy (base units): 0.01 USDC ..= 1,000 USDC.
const MIN_ORDER_BASE: i64 = 10_000;
const MAX_ORDER_BASE: i64 = 1_000_000_000;

/// Convert our domain side to Panta's wire string ("yes"/"no").
fn side_to_wire(side: Side) -> String {
    side.as_str().to_string()
}

/// Map a stored status string -> API enum (defensive default: Quoted).
fn parse_status(s: &str) -> OrderStatus {
    match s {
        "quoted" => OrderStatus::Quoted,
        "built" => OrderStatus::Built,
        "submitted" => OrderStatus::Submitted,
        "confirmed" => OrderStatus::Confirmed,
        "filled" => OrderStatus::Filled,
        _ => OrderStatus::Failed,
    }
}

/// `POST /v1/orders/quote` — simulate a fill at Panta and persist the order.
pub async fn quote(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<OrderQuoteRequest>,
) -> Result<Json<OrderQuote>, AppError> {
    // 1. Validate input before it leaves the app.
    let amount = req
        .amount_usdc
        .parse::<i64>()
        .map_err(|_| AppError::bad_request("INVALID_AMOUNT", "amount must be an integer string of base units"))?;
    if !(MIN_ORDER_BASE..=MAX_ORDER_BASE).contains(&amount) {
        return Err(AppError::bad_request(
            "AMOUNT_OUT_OF_RANGE",
            "amount must be between 0.01 and 1000 USDC",
        ));
    }

    // 2. Ask Panta to simulate the fill (wallet comes from the session).
    let quote = state
        .panta
        .quote_primary(&panta::QuoteRequest {
            wallet: user.wallet.clone(),
            market_id: req.market_id.clone(),
            side: side_to_wire(req.side),
            amount_usdc: panta::usdc_base_to_human(amount),
            user_id: None,
        })
        .await?;

    // 3. Persist as an order row (status = quoted). Our order id is what the
    //    frontend uses; Panta's quoteId is kept server-side.
    let shares_base = panta::usdc_human_to_base(&quote.shares)
        .map_err(|e| AppError::internal(format!("panta returned an unparseable share count: {e}")))?;
    let fee_base = panta::usdc_human_to_base(&quote.fee_usdc).unwrap_or(0);

    let mut client = state.db.get().await?;
    let order_id = queries::orders::insert_order(
        &client,
        user.user_id,
        &req.market_id,
        &side_to_wire(req.side),
        amount,
        Some(shares_base),
        &quote.quote_id,
    )
    .await?;

    Ok(Json(OrderQuote {
        quote_id: order_id.to_string(),
        market_id: quote.market_id,
        side: req.side,
        amount_usdc: amount.to_string(),
        expected_shares: shares_base.to_string(),
        avg_price: quote.avg_price,
        panta_fee_usdc: fee_base.to_string(),
        our_fee_usdc: "0".to_string(), // 0 bps on single-market buys (MVP)
        price_impact_bps: None,        // not provided by Panta's quote
        expires_at: quote.expires_at,
    }))
}

/// `POST /v1/orders/{quoteId}/build` — assemble the real transaction.
///
/// Pipeline: Panta build (instructions) -> our solana layer assembles ONE
/// unsigned v0 tx with ComputeBudget instructions -> safety-validate ->
/// simulate (real mode) -> return base64 + a plain-language summary.
pub async fn build(
    State(state): State<AppState>,
    user: AuthUser,
    Path(quote_id): Path<String>,
) -> Result<Json<BuildResponse>, AppError> {
    let order_id = parse_order_id(&quote_id)?;
    let mut client = state.db.get().await?;

    // 1. Load the order and enforce ownership + state.
    let order = load_owned_order(&client, order_id, user.user_id).await?;
    if order.status != "quoted" {
        return Err(AppError::bad_request(
            "ORDER_NOT_QUOTED",
            format!("order is in status '{0}', only quoted orders can be built", order.status),
        ));
    }
    let panta_quote_id = order
        .panta_quote_id
        .clone()
        .ok_or_else(|| AppError::internal("order is missing its panta quote id"))?;

    // 2. Panta builds the buy instructions from the live quote session.
    let built = state
        .panta
        .build_primary(&panta::BuildRequest {
            quote_id: panta_quote_id,
            wallet: user.wallet.clone(),
            user_id: None,
            max_slippage_bps: Some(state.config.default_slippage_bps),
        })
        .await?;

    // 3. Convert wire instructions and assemble the unsigned transaction.
    //    Single-market buys: our fee is 0, so no fee instruction is added.
    let payer = parse_pubkey(&user.wallet)?;
    let mut legs = Vec::with_capacity(built.instructions.len());
    for wire in &built.instructions {
        legs.push(vec![SolanaCtx::parse_wire_instruction(wire)?]);
    }
    let tx = state.solana.assemble(&payer, legs, None, 400_000).await?;

    // 4. Funds-protection check + on-chain simulation BEFORE the user signs.
    state.solana.validate(&tx, &payer, None)?;
    state.solana.simulate(&tx).await?;

    // 5. Persist the built state with Panta's order id for submit/verify.
    queries::orders::mark_built(&client, order_id, &built.order_id).await?;

    // 6. Encode for the wallet (standard base64 of the wire serialization).
    let tx_base64 = encode_tx(&tx)?;

    let summary = TxSummary {
        summary_lines: vec![format!(
            "You are buying {} on this market for {} USDC.",
            match order.side.as_str() {
                "yes" => "YES",
                _ => "NO",
            },
            panta::usdc_base_to_human(order.amount_usdc),
        )],
        total_usdc: order.amount_usdc.to_string(),
        our_fee_usdc: "0".to_string(),
        panta_fees_usdc: built
            .fee_usdc
            .as_deref()
            .and_then(|f| panta::usdc_human_to_base(f).ok())
            .unwrap_or(0)
            .to_string(),
        notices: vec![
            "You cannot sell before this market resolves.".to_string(),
            "Powered by Panta.".to_string(),
        ],
    };

    Ok(Json(BuildResponse {
        order_id,
        transactions: vec![tx_base64],
        summary,
        expires_at: built.expires_at.unwrap_or_else(|| Utc::now() + chrono::Duration::seconds(60)),
    }))
}

/// `POST /v1/orders/{orderId}/submit` — the user signed; broadcast it.
///
/// Returns as soon as the transaction is on the network; confirmation and
/// Panta attribution happen right after (inline await in mock mode is instant;
/// in real mode the broadcast call itself retries, and confirmation is polled
/// by this request — bounded at ~90 s worst case).
pub async fn submit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(order_id_path): Path<String>,
    Json(req): Json<SubmitRequest>,
) -> Result<Json<OrderView>, AppError> {
    let order_id = parse_order_id(&order_id_path)?;
    let mut client = state.db.get().await?;

    // 1. Load + ownership + state.
    let order = load_owned_order(&client, order_id, user.user_id).await?;
    if order.status != "built" {
        return Err(AppError::bad_request(
            "ORDER_NOT_BUILT",
            format!("order is in status '{0}', build it first", order.status),
        ));
    }
    let panta_order_id = order
        .panta_order_id
        .clone()
        .ok_or_else(|| AppError::internal("order is missing its panta order id"))?;

    if req.signed_txs.len() != 1 {
        return Err(AppError::bad_request(
            "INVALID_REQUEST",
            "single-market orders require exactly one signed transaction",
        ));
    }

    // 2. Deserialize the signed transaction (user's wallet filled the sigs).
    let signed: solana_sdk::transaction::VersionedTransaction =
        bincode::deserialize(&decode_b64(&req.signed_txs[0])?)
            .map_err(|_| AppError::bad_request("INVALID_TX", "signed transaction could not be decoded"))?;

    // 3. Broadcast through our RPC and wait for confirmation.
    let signature = state.solana.broadcast_and_confirm(&signed).await?;

    // 4. Record the signature, then tell Panta (idempotent on both sides).
    queries::orders::mark_submitted(&client, order_id, &signature).await?;
    queries::orders::insert_transaction(&client, &signature, user.user_id, "order").await?;
    queries::orders::update_transaction_status(&client, &signature, "confirmed", None).await?;

    let submit_result = state
        .panta
        .submit_primary(&panta::SubmitRequest {
            order_id: panta_order_id.clone(),
            signature: signature.clone(),
            wallet: Some(user.wallet.clone()),
        })
        .await;

    // Attribution report: primary buy -> kind buy (idempotent per signature).
    let _ = state
        .panta
        .report_trade(&panta::ReportTradeRequest {
            signature: signature.clone(),
            wallet: user.wallet.clone(),
            market_id: order.market_id.clone(),
            quote_id: order.panta_quote_id.clone(),
            client_order_id: Some(order_id.to_string()),
            user_id: None,
        })
        .await;

    // 5. Panta accept or not, the on-chain truth stands: mark filled.
    if let Err(e) = submit_result {
        tracing::warn!(error = %e, "panta submit failed (on-chain purchase still stands)");
    }
    queries::orders::set_status(&client, order_id, "filled", None).await?;

    Ok(Json(order_view(order_id, user.user_id, &client).await?))
}

/// `GET /v1/orders/{orderId}` — poll order status from the frontend.
pub async fn status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(order_id_path): Path<String>,
) -> Result<Json<OrderView>, AppError> {
    let order_id = parse_order_id(&order_id_path)?;
    let client = state.db.get().await?;
    let view = order_view(order_id, user.user_id, &client).await?;
    Ok(Json(view))
}

// --- shared helpers ----------------------------------------------------------

fn parse_order_id(path: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(path)
        .map_err(|_| AppError::bad_request("INVALID_ID", "order id must be a uuid"))
}

fn parse_pubkey(wallet: &str) -> Result<solana_sdk::pubkey::Pubkey, AppError> {
    solana_sdk::pubkey::Pubkey::from_str(wallet)
        .map_err(|_| AppError::bad_request("INVALID_WALLET", "wallet is not a valid pubkey"))
}

/// Load an order and 404 unless it exists AND belongs to the caller.
/// (No information leak: other users' orders simply do not exist to you.)
async fn load_owned_order(
    client: &deadpool_postgres::Client,
    order_id: Uuid,
    user_id: Uuid,
) -> Result<queries::orders::OrderRow, AppError> {
    let order = queries::orders::get_order(client, order_id)
        .await?
        .filter(|o| o.user_id == user_id)
        .ok_or_else(|| AppError::not_found("order not found"))?;
    Ok(order)
}

/// Standard base64 of the bincode/wire serialization — what wallets expect.
pub fn encode_tx(tx: &solana_sdk::transaction::VersionedTransaction) -> Result<String, AppError> {
    let bytes = bincode::serialize(tx)
        .map_err(|e| AppError::internal(format!("could not serialize transaction: {e}")))?;
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes))
}

fn decode_b64(s: &str) -> Result<Vec<u8>, AppError> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| AppError::bad_request("INVALID_TX", "transaction is not valid base64"))
}

/// DB row -> API view.
async fn order_view(order_id: Uuid, user_id: Uuid, client: &deadpool_postgres::Client) -> Result<OrderView, AppError> {
    let order = queries::orders::get_order(client, order_id)
        .await?
        .filter(|o| o.user_id == user_id)
        .ok_or_else(|| AppError::not_found("order not found"))?;
    Ok(OrderView {
        order_id: order.id,
        market_id: order.market_id,
        market_title: order.market_title,
        side: match order.side.as_str() {
            "yes" => Side::Yes,
            _ => Side::No,
        },
        amount_usdc: order.amount_usdc.to_string(),
        expected_shares: order.expected_shares.map(|s| s.to_string()),
        status: parse_status(&order.status),
        tx_signature: order.tx_signature,
        error_code: order.error_code,
        created_at: order.created_at,
        updated_at: order.updated_at,
    })
}

/// Map transaction-layer failures onto the API error envelope (brief §9 codes).
pub fn tx_error_to_app(e: TxError) -> AppError {
    match e {
        TxError::TooLarge { .. } => AppError::bad_request("TX_TOO_LARGE", e.to_string()),
        TxError::Validation(_) => AppError::bad_request("TX_VALIDATION_FAILED", e.to_string()),
        TxError::Simulation(_) => AppError::bad_request("SIMULATION_FAILED", e.to_string()),
        TxError::Invalid(_) => AppError::bad_request("INVALID_TX", e.to_string()),
        TxError::Rpc(_) => AppError::new(axum::http::StatusCode::BAD_GATEWAY, "RPC_UNAVAILABLE", e.to_string()),
        TxError::Expired | TxError::NotConfirmed => AppError::new(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            "TX_CONFIRM_TIMEOUT",
            e.to_string(),
        ),
    }
}

// TxError -> AppError for `?` in handlers (funds-protection errors stay loud).
impl From<TxError> for AppError {
    fn from(e: TxError) -> Self {
        tx_error_to_app(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tx error mapping must keep our documented codes.
    #[test]
    fn tx_errors_map_to_api_codes() {
        let e = tx_error_to_app(TxError::TooLarge { size: 2000 });
        assert_eq!(e.code, "TX_TOO_LARGE");
        let e = tx_error_to_app(TxError::Validation("rogue program".into()));
        assert_eq!(e.code, "TX_VALIDATION_FAILED");
        let e = tx_error_to_app(TxError::Simulation("slippage".into()));
        assert_eq!(e.code, "SIMULATION_FAILED");
    }

    /// Transaction serialization round trip: base64 -> deserialize -> re-encode.
    #[test]
    fn tx_encoding_round_trips() {
        let user = solana_sdk::pubkey::Pubkey::new_unique();
        let ctx = test_ctx();
        let tx = ctx
            .assemble_with_blockhash(
                &user,
                vec![vec![solana_sdk::instruction::Instruction {
                    program_id: solana_sdk::pubkey::Pubkey::from_str(solana_tx::SYSTEM_PROGRAM).unwrap(),
                    accounts: vec![solana_sdk::instruction::AccountMeta::new_readonly(user, true)],
                    data: vec![1],
                }]],
                None,
                100_000,
                solana_sdk::hash::Hash::default(),
            )
            .unwrap();

        let b64 = encode_tx(&tx).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let back: solana_sdk::transaction::VersionedTransaction =
            bincode::deserialize(&bytes).unwrap();
        assert_eq!(back.message.hash(), tx.message.hash());
    }

    /// Minimal SolanaCtx for pure assembly tests (mock mode, no RPC calls).
    fn test_ctx() -> SolanaCtx {
        let config = crate::config::Config {
            app_env: "development".into(),
            database_url: "postgres://x".into(),
            panta_api_base_url: "https://example.test".into(),
            panta_api_key: String::new(),
            panta_mock: true,
            solana_rpc_url: "http://localhost:8899".into(),
            solana_ws_url: None,
            usdc_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
            fee_wallet_pubkey: "1FeeWa11et111111111111111111111111111111111".into(),
            app_fee_bps: 75,
            max_basket_legs: 4,
            default_slippage_bps: 150,
            priority_fee_microlamports: 5000,
            panta_program_ids: vec![],
            jwt_secret: "test".into(),
            admin_wallets: vec![],
            blocked_countries: vec![],
            cors_origins: vec![],
            api_bind: "0.0.0.0:8080".into(),
        };
        SolanaCtx::from_config(&config).unwrap()
    }
}
