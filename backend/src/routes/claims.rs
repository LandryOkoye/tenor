//! Win-claim endpoints (brief §7.7, §9): collect winnings on resolved
//! markets, one claim or "claim all", signed by the user's wallet.
//!
//! Each built claim transaction is recorded as a `claim_batches` row with
//! the exact message bytes; submit only accepts that transaction, and the
//! batch's market list (not a guess from positions) drives bookkeeping.

use axum::{extract::State, Json};
use std::str::FromStr;

use crate::auth::AuthUser;
use crate::domain::{
    ClaimsBuildRequest, ClaimsBuildResponse, ClaimsSubmitRequest, ClaimsSubmitResponse, TxSummary,
};
use crate::error::AppError;
use crate::panta;
use crate::queries;
use crate::routes::markets::check_market_id;
use crate::routes::orders::encode_tx;
use crate::settlement;
use crate::solana_tx::{SolanaCtx, TxError};
use crate::state::AppState;

/// Compute budget for a claim transaction.
const CLAIM_CU_LIMIT: u32 = 200_000;
/// Upper bound on markets considered per build call (keeps Panta build
/// budget usage bounded; the rest come back in `remaining_market_ids`).
const MAX_CLAIMS_PER_BUILD: usize = 8;

/// `POST /v1/claims/build` — assemble one claim transaction.
///
/// `market_ids` empty = claim all (every claimable position of the wallet).
/// Claims are packed into one transaction until it is full; whatever does
/// not fit is returned in `remaining_market_ids`.
pub async fn build(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<ClaimsBuildRequest>,
) -> Result<Json<ClaimsBuildResponse>, AppError> {
    // 1. Which markets? Explicit list, or every claimable position.
    let mut market_ids: Vec<String> = if req.market_ids.is_empty() {
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
    market_ids.sort();
    market_ids.dedup();
    for id in &market_ids {
        check_market_id(id)?;
    }
    if market_ids.is_empty() {
        return Err(AppError::bad_request(
            "NOTHING_CLAIMABLE",
            "no claimable positions for this wallet",
        ));
    }
    let payer = solana_sdk::pubkey::Pubkey::from_str(&user.wallet)
        .map_err(|_| AppError::bad_request("INVALID_WALLET", "wallet is not a valid pubkey"))?;

    // 2. Build claim instructions market by market (Panta fails closed per
    //    market: NOT_CLAIMABLE surfaces as a clear error) and keep adding
    //    while the transaction still fits in one packet.
    let mut included: Vec<String> = Vec::new();
    let mut instruction_sets = Vec::new();
    let mut total_claimable: i64 = 0;
    for market_id in market_ids.iter().take(MAX_CLAIMS_PER_BUILD) {
        let claim = state
            .panta
            .build_claim(&panta::ClaimBuildRequest {
                wallet: user.wallet.clone(),
                market_id: market_id.clone(),
            })
            .await?;
        let mut ixs = Vec::with_capacity(claim.instructions.len());
        for wire in &claim.instructions {
            ixs.push(SolanaCtx::parse_wire_instruction(wire)?);
        }

        // Size probe with a placeholder blockhash (size does not depend on it).
        let mut candidate = instruction_sets.clone();
        candidate.push(ixs.clone());
        match state.solana.assemble_with_blockhash(
            &payer,
            candidate,
            None,
            CLAIM_CU_LIMIT,
            solana_sdk::hash::Hash::default(),
        ) {
            Ok(_) => {}
            Err(TxError::TooLarge { .. }) if !included.is_empty() => break,
            Err(e) => return Err(e.into()),
        }

        instruction_sets.push(ixs);
        included.push(market_id.clone());
        // ≈ 1 USDC per winning share
        total_claimable +=
            panta::usdc_human_to_base(claim.winning_shares.as_deref().unwrap_or("0")).unwrap_or(0);
    }
    let remaining: Vec<String> = market_ids
        .iter()
        .filter(|m| !included.contains(m))
        .cloned()
        .collect();

    // 3. One transaction, no fee of ours (claims pay us nothing); validate
    //    and simulate before the user sees it.
    let assembled = state
        .solana
        .assemble(&payer, instruction_sets, None, CLAIM_CU_LIMIT)
        .await?;
    state.solana.validate(&assembled.tx, &payer, None)?;
    state.solana.simulate(&assembled.tx).await?;

    // 4. Record exactly what the user will sign.
    let client = state.db.get().await?;
    let claim_id = queries::portfolio::insert_claim_batch(
        &client,
        user.user_id,
        &included,
        &assembled.message_bytes(),
        assembled.expiry(),
    )
    .await?;

    let n = included.len();
    Ok(Json(ClaimsBuildResponse {
        claim_id,
        market_ids: included,
        remaining_market_ids: remaining,
        transactions: vec![encode_tx(&assembled.tx)?],
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

/// `POST /v1/claims/submit` — broadcast the signed claim and attribute it.
pub async fn submit(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<ClaimsSubmitRequest>,
) -> Result<Json<ClaimsSubmitResponse>, AppError> {
    if req.signed_txs.len() != 1 {
        return Err(AppError::bad_request(
            "INVALID_REQUEST",
            "a claim batch is exactly one transaction",
        ));
    }

    let (signed, signature) = {
        let client = state.db.get().await?;
        let batch = queries::portfolio::get_claim_batch(&client, req.claim_id)
            .await?
            .filter(|b| b.user_id == user.user_id)
            .ok_or_else(|| AppError::not_found("claim not found"))?;
        if batch.status != "built" {
            return Err(AppError::bad_request(
                "CLAIM_NOT_BUILT",
                format!("claim is in status '{}'", batch.status),
            ));
        }

        // Must be exactly the claim tx we built, validly signed.
        let signed = settlement::decode_signed(&req.signed_txs[0])?;
        let signature = settlement::verify_signed(&signed, &batch.built_message)?;

        settlement::record_submission(
            &client,
            "claim",
            batch.id,
            user.user_id,
            &signature,
            batch.last_valid_block_height,
        )
        .await?;
        (signed, signature)
    };

    settlement::broadcast_and_settle(&state, &signed, &signature).await?;

    let client = state.db.get().await?;
    let status = queries::portfolio::get_claim_batch(&client, req.claim_id)
        .await?
        .map(|b| b.status)
        .unwrap_or_else(|| "submitted".into());
    Ok(Json(ClaimsSubmitResponse {
        claim_id: req.claim_id,
        signatures: vec![signature],
        status,
    }))
}
