//! Settlement: what happens to a user-signed transaction after we get it.
//!
//! Shared by the submit endpoints (orders, baskets, claims) and the
//! `tx_confirm` worker job, so a purchase finishes the same way whether the
//! HTTP request saw the confirmation or the request died first.
//!
//! Lifecycle of every signed transaction:
//! 1. `check_signed` — byte-identical to the message we built + valid sigs
//! 2. record the signature (`transactions` row + record -> `submitted`)
//!    BEFORE broadcasting, so a landed tx can never be lost
//! 3. broadcast, then wait briefly for `confirmed`
//! 4. `finalize` (Panta submit + attribution, record -> filled/confirmed) or
//!    `fail`; anything still pending is finished by the `tx_confirm` job
//!
//! Every step is idempotent: state moves are conditional on the current
//! status, Panta submit/report are idempotent per signature.

use std::time::Duration;

use uuid::Uuid;

use crate::error::AppError;
use crate::panta;
use crate::queries;
use crate::solana_tx::{SigStatus, SolanaCtx, TxError};
use crate::state::AppState;

/// How long a submit request waits for confirmation before handing the
/// signature to the worker and returning `submitted`.
pub const INLINE_CONFIRM_WAIT: Duration = Duration::from_secs(20);

/// Decode a base64 wire transaction from the client.
pub fn decode_signed(
    b64: &str,
) -> Result<solana_sdk::transaction::VersionedTransaction, crate::error::AppError> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|_| {
            crate::error::AppError::bad_request("INVALID_TX", "transaction is not valid base64")
        })?;
    bincode::deserialize(&bytes).map_err(|_| {
        crate::error::AppError::bad_request("INVALID_TX", "signed transaction could not be decoded")
    })
}

/// Verify a signed tx against the stored message and return its signature.
pub fn verify_signed(
    signed: &solana_sdk::transaction::VersionedTransaction,
    built_message: &[u8],
) -> Result<String, TxError> {
    SolanaCtx::check_signed(signed, built_message)?;
    SolanaCtx::signature_of(signed)
}

/// Atomically record a submission before anything touches the network:
/// a new `transactions` row for the signature AND the record's move from
/// `built` to `submitted`. Rejects a signature we have seen before and a
/// record that was already submitted.
pub async fn record_submission(
    client: &deadpool_postgres::Client,
    kind: &str, // order | basket | claim
    ref_id: Uuid,
    user_id: Uuid,
    signature: &str,
    last_valid_block_height: Option<i64>,
) -> Result<(), AppError> {
    client.batch_execute("BEGIN").await?;
    let result: Result<(), AppError> = async {
        if !queries::orders::insert_transaction(
            client,
            signature,
            user_id,
            kind,
            ref_id,
            last_valid_block_height,
        )
        .await?
        {
            return Err(AppError::bad_request(
                "DUPLICATE_TX",
                "this signed transaction was already submitted",
            ));
        }
        let moved = match kind {
            "order" => queries::orders::mark_submitted(client, ref_id, signature).await?,
            "basket" => {
                queries::baskets::mark_purchase_submitted(client, ref_id, signature).await?
            }
            "claim" => {
                queries::portfolio::transition_claim_batch(
                    client,
                    ref_id,
                    "built",
                    "submitted",
                    Some(signature),
                    None,
                )
                .await?
            }
            other => return Err(AppError::internal(format!("unknown tx kind {other}"))),
        };
        if !moved {
            return Err(AppError::bad_request(
                "ALREADY_SUBMITTED",
                "this transaction was already submitted",
            ));
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => {
            client.batch_execute("COMMIT").await?;
            Ok(())
        }
        Err(e) => {
            client.batch_execute("ROLLBACK").await?;
            Err(e)
        }
    }
}

/// Broadcast an already-recorded signed tx and wait briefly for an outcome.
/// Settles the record when the outcome is known; otherwise the tx stays
/// `submitted` for the worker. Broadcast errors are returned, but the record
/// stays `submitted` — the tx may still land, and the worker expires it if not.
pub async fn broadcast_and_settle(
    state: &AppState,
    signed: &solana_sdk::transaction::VersionedTransaction,
    signature: &str,
) -> Result<(), TxError> {
    state.solana.send(signed).await?;
    match state.solana.wait_for(signature, INLINE_CONFIRM_WAIT).await {
        SigStatus::Confirmed => {
            if let Err(e) = finalize(state, signature).await {
                tracing::error!(%signature, error = ?e, "finalize failed; worker will retry");
            }
        }
        SigStatus::Failed(err) => {
            if let Err(e) = fail(state, signature, "TX_FAILED", &err).await {
                tracing::error!(%signature, error = ?e, "fail-settlement failed; worker will retry");
            }
        }
        SigStatus::Unknown | SigStatus::Processed => {
            tracing::info!(%signature, "not confirmed yet; tx_confirm job will finish it");
        }
    }
    Ok(())
}

/// Check a pending signature once and settle it if its fate is known.
/// Used by the `tx_confirm` job. Returns true when the tx was settled.
pub async fn reconcile(
    state: &AppState,
    pending: &queries::orders::PendingTx,
    block_height: u64,
) -> anyhow::Result<bool> {
    match state.solana.signature_status(&pending.signature).await? {
        SigStatus::Confirmed => {
            finalize(state, &pending.signature).await?;
            Ok(true)
        }
        SigStatus::Failed(err) => {
            fail(state, &pending.signature, "TX_FAILED", &err).await?;
            Ok(true)
        }
        // Never seen, and the blockhash is past expiry: it can never land.
        SigStatus::Unknown
            if pending
                .last_valid_block_height
                .is_some_and(|h| block_height > h as u64) =>
        {
            fail(
                state,
                &pending.signature,
                "TX_EXPIRED",
                "blockhash expired before landing",
            )
            .await?;
            Ok(true)
        }
        SigStatus::Unknown | SigStatus::Processed => Ok(false),
    }
}

/// The tx confirmed on-chain: tell Panta, then settle our records.
pub async fn finalize(state: &AppState, signature: &str) -> anyhow::Result<()> {
    let client = state.db.get().await?;
    let Some(tx) = queries::orders::get_transaction(&client, signature).await? else {
        anyhow::bail!("unknown signature {signature}");
    };
    let ref_id = tx
        .ref_id
        .ok_or_else(|| anyhow::anyhow!("transaction {signature} has no ref_id"))?;

    match tx.kind.as_str() {
        "order" => {
            let order = queries::orders::get_order(&client, ref_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("order {ref_id} missing"))?;
            let wallet = wallet(&client, order.user_id).await?;
            if let Some(panta_order_id) = &order.panta_order_id {
                attribute_buy(state, &wallet, &order, panta_order_id, signature).await;
            }
            queries::orders::transition(&client, ref_id, "submitted", "filled", None).await?;
        }
        "basket" => {
            let purchase = queries::baskets::get_purchase(&client, ref_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("purchase {ref_id} missing"))?;
            let wallet = wallet(&client, purchase.user_id).await?;
            for leg in queries::baskets::purchase_leg_orders(&client, ref_id).await? {
                if let Some(panta_order_id) = &leg.panta_order_id {
                    attribute_buy(state, &wallet, &leg, panta_order_id, signature).await;
                }
            }
            queries::baskets::settle_purchase(&client, ref_id, "filled", None).await?;
        }
        "claim" => {
            let batch = queries::portfolio::get_claim_batch(&client, ref_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("claim batch {ref_id} missing"))?;
            let wallet = wallet(&client, batch.user_id).await?;
            for market_id in &batch.market_ids {
                queries::portfolio::insert_claim(
                    &client,
                    batch.user_id,
                    market_id,
                    signature,
                    None,
                )
                .await?;
                queries::portfolio::mark_orders_claimed(&client, batch.user_id, market_id).await?;
                if let Err(e) = state
                    .panta
                    .report_trade(&panta::ReportTradeRequest {
                        signature: signature.to_string(),
                        wallet: wallet.clone(),
                        market_id: market_id.clone(),
                        quote_id: None,
                        client_order_id: None,
                        user_id: None,
                    })
                    .await
                {
                    tracing::warn!(%signature, market = %market_id, error = %e, "claim attribution failed");
                }
            }
            queries::portfolio::transition_claim_batch(
                &client,
                ref_id,
                "submitted",
                "confirmed",
                None,
                None,
            )
            .await?;
        }
        other => anyhow::bail!("unknown transaction kind {other}"),
    }

    queries::orders::update_transaction_status(&client, signature, "confirmed", None).await?;
    tracing::info!(%signature, kind = %tx.kind, "transaction settled");
    Ok(())
}

/// The tx failed or expired: nothing was bought/claimed. Mark records failed.
pub async fn fail(
    state: &AppState,
    signature: &str,
    code: &str,
    detail: &str,
) -> anyhow::Result<()> {
    let client = state.db.get().await?;
    let Some(tx) = queries::orders::get_transaction(&client, signature).await? else {
        anyhow::bail!("unknown signature {signature}");
    };
    if let Some(ref_id) = tx.ref_id {
        match tx.kind.as_str() {
            "order" => {
                queries::orders::transition(&client, ref_id, "submitted", "failed", Some(code))
                    .await?;
            }
            "basket" => {
                queries::baskets::settle_purchase(&client, ref_id, "failed", Some(code)).await?;
            }
            "claim" => {
                queries::portfolio::transition_claim_batch(
                    &client,
                    ref_id,
                    "submitted",
                    "failed",
                    None,
                    Some(code),
                )
                .await?;
            }
            _ => {}
        }
    }
    queries::orders::update_transaction_status(&client, signature, "failed", Some(detail)).await?;
    tracing::warn!(%signature, code, detail, "transaction failed");
    Ok(())
}

async fn wallet(client: &deadpool_postgres::Client, user_id: Uuid) -> anyhow::Result<String> {
    queries::users::wallet_of(client, user_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("user {user_id} missing"))
}

/// Panta submit + attribution report for one bought leg. Both are
/// idempotent per (order, signature); failures are logged, never fatal —
/// the on-chain purchase stands regardless.
async fn attribute_buy(
    state: &AppState,
    wallet: &str,
    order: &queries::orders::OrderRow,
    panta_order_id: &str,
    signature: &str,
) {
    if let Err(e) = state
        .panta
        .submit_primary(&panta::SubmitRequest {
            order_id: panta_order_id.to_string(),
            signature: signature.to_string(),
            wallet: Some(wallet.to_string()),
        })
        .await
    {
        tracing::warn!(%signature, order = %order.id, error = %e, "panta submit failed");
    }
    if let Err(e) = state
        .panta
        .report_trade(&panta::ReportTradeRequest {
            signature: signature.to_string(),
            wallet: wallet.to_string(),
            market_id: order.market_id.clone(),
            quote_id: order.panta_quote_id.clone(),
            client_order_id: Some(order.id.to_string()),
            user_id: None,
        })
        .await
    {
        tracing::warn!(%signature, order = %order.id, error = %e, "panta attribution failed");
    }
}
