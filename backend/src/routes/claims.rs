//! Win-claim endpoints (brief §7.7, §9): collect winnings on resolved
//! markets, one claim or "claim all", signed by the user's wallet.

use axum::{extract::State, Json};
use std::str::FromStr;

use crate::auth::AuthUser;
use crate::domain::{ClaimsBuildRequest, ClaimsBuildResponse, ClaimsSubmitResponse, TxSummary};
use crate::error::AppError;
use crate::panta::{self, PantaApi};
use crate::queries;
use crate::routes::orders::encode_tx;
use crate::state::AppState;

/// `POST /v1/claims/build` — assemble claim transactions.
///
/// `market_ids` empty = claim all (every claimable position of the wallet).
/// All claim instructions are packed into ONE transaction (claims are tiny).
pub async fn build(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<ClaimsBuildRequest>,
) -> Result<Json<ClaimsBuildResponse>, AppError> {
    // 1. Which markets? Explicit list, or every claimable position.
    let market_ids: Vec<String> = if req.market_ids.is_empty() {
        state
            .panta
            .positions(&user.wallet)
            .await?
            .into_iter()
            .filter(|p| p.claimable)
            .map(|p| p.market_id)
            .collect()
    } else {
        req.market_ids
    };
    if market_ids.is_empty() {
        return Err(AppError::bad_request(
            "NOTHING_CLAIMABLE",
            "no claimable positions for this wallet",
        ));
    }

    // 2. Build claim instructions for each market (Panta fails closed per
    //    market: NOT_CLAIMABLE surfaces as a clear error).
    let mut instruction_sets = Vec::with_capacity(market_ids.len());
    let mut total_claimable: i64 = 0;
    for market_id in &market_ids {
        let claim = state
            .panta
            .build_claim(&panta::ClaimBuildRequest {
                wallet: user.wallet.clone(),
                market_id: market_id.clone(),
            })
            .await?;
        total_claimable += panta::usdc_human_to_base(
            claim.winning_shares.as_deref().unwrap_or("0"),
        )
        .unwrap_or(0); // ≈ 1 USDC per winning share
        let mut ixs = Vec::with_capacity(claim.instructions.len());
        for wire in &claim.instructions {
            ixs.push(crate::solana_tx::SolanaCtx::parse_wire_instruction(wire)?);
        }
        instruction_sets.push(ixs);
    }

    // 3. One transaction, no our-fee (claims pay nothing to us), simulate it.
    let payer = solana_sdk::pubkey::Pubkey::from_str(&user.wallet)
        .map_err(|_| AppError::bad_request("INVALID_WALLET", "wallet is not a valid pubkey"))?;
    let tx = state.solana.assemble(&payer, instruction_sets, None, 200_000).await?;
    state.solana.validate(&tx, &payer, None)?;
    state.solana.simulate(&tx).await?;

    let n = market_ids.len();
    Ok(Json(ClaimsBuildResponse {
        transactions: vec![encode_tx(&tx)?],
        summary: TxSummary {
            summary_lines: vec![format!(
                "You are claiming winnings on {n} market{} (~{} USDC).",
                if n == 1 { "" } else { "s" },
                panta::usdc_base_to_human(total_claimable),
            )],
            total_usdc: total_claimable.to_string(),
            our_fee_usdc: "0".to_string(),
            panta_fees_usdc: "0".to_string(),
            notices: vec!["Powered by Panta.".to_string()],
        },
        total_claimable_usdc: total_claimable.to_string(),
    }))
}

/// `POST /v1/claims/submit` — broadcast the signed claim(s) and attribute.
pub async fn submit(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<crate::domain::SubmitRequest>,
) -> Result<Json<ClaimsSubmitResponse>, AppError> {
    let client = state.db.get().await?;
    if req.signed_txs.len() != 1 {
        return Err(AppError::bad_request(
            "INVALID_REQUEST",
            "claims are packed into exactly one transaction",
        ));
    }

    // 1. Decode, broadcast, confirm.
    let signed: solana_sdk::transaction::VersionedTransaction =
        bincode::deserialize(&decode_b64(&req.signed_txs[0])?)
            .map_err(|_| AppError::bad_request("INVALID_TX", "signed transaction could not be decoded"))?;
    let signature = state.solana.broadcast_and_confirm(&signed).await?;

    // 2. Which markets were claimed? Whatever the wallet held claimable.
    let claimed_markets: Vec<String> = state
        .panta
        .positions(&user.wallet)
        .await?
        .into_iter()
        .filter(|p| p.claimable || p.claimed.unwrap_or(false))
        .map(|p| p.market_id)
        .collect();

    // 3. Bookkeeping: claims rows, leg orders marked claimed, Panta attribution.
    for market_id in &claimed_markets {
        queries::portfolio::insert_claim(
            &client,
            user.user_id,
            market_id,
            &signature,
            None, // amount reconciled by the worker later; the tx is the truth
        )
        .await?;
        queries::portfolio::mark_orders_claimed(&client, market_id).await?;
        let _ = state
            .panta
            .report_trade(&panta::ReportTradeRequest {
                signature: signature.clone(),
                wallet: user.wallet.clone(),
                market_id: market_id.clone(),
                quote_id: None,
                client_order_id: None,
                user_id: None,
            })
            .await;
    }
    queries::orders::insert_transaction(&client, &signature, user.user_id, "claim").await?;
    queries::orders::update_transaction_status(&client, &signature, "confirmed", None).await?;

    Ok(Json(ClaimsSubmitResponse {
        signatures: vec![signature],
        status: "confirmed".to_string(),
    }))
}

fn decode_b64(s: &str) -> Result<Vec<u8>, AppError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| AppError::bad_request("INVALID_TX", "transaction is not valid base64"))
}
