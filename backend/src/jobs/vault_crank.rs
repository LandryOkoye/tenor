//! `vault_crank` — claim winning legs of tokenized baskets into their vaults
//! (ADR 0002). For every live/closed vault basket, each leg whose market
//! resolved in the leg's favour and has no `WinClaim` receipt yet gets a
//! `claim_leg` transaction, paid by the crank keypair (`CRANK_KEYPAIR`).
//! The program sends the payout to the vault only, so the crank can never
//! redirect funds. Idempotent: a claimed leg has a receipt and is skipped.
//!
//! Once every leg resolved and the grace period passed, the crank also sends
//! the permissionless `settle_after_grace`, which opens redemption.

use std::str::FromStr;

use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::VersionedTransaction,
};

use crate::queries;
use crate::state::AppState;
use crate::vault::{self, VaultLeg};

const CLAIM_CU: u32 = 300_000;

pub async fn run_once(state: AppState) -> anyhow::Result<()> {
    let (Some(program), Some(keypair_path)) = (
        state.config.vault_program_id.as_deref(),
        state.config.crank_keypair.as_deref(),
    ) else {
        return Ok(()); // crank disabled
    };
    if state.solana.is_mock() {
        return Ok(());
    }
    let program = Pubkey::from_str(program)?;
    let usdc = Pubkey::from_str(&state.config.usdc_mint)?;
    let crank = read_keypair_file(keypair_path)
        .map_err(|e| anyhow::anyhow!("CRANK_KEYPAIR unreadable: {e}"))?;

    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT template_id FROM vault_baskets WHERE status IN ('live', 'closed')",
            &[],
        )
        .await?;
    let mut claimed = 0usize;
    for r in rows {
        let Some(basket) = queries::vault::get_basket(&client, r.get(0)).await? else {
            continue;
        };
        let legs: Vec<VaultLeg> = serde_json::from_value(basket.legs_json.clone())?;
        let a = vault::addresses(&program, basket.onchain_id as u64, &usdc);
        for (i, leg) in legs.iter().enumerate() {
            // Our catalog says resolved, and in this leg's favour?
            let Some(m) = queries::markets::get_market(&client, &leg.market_id).await? else {
                continue;
            };
            let side = if leg.side == 0 { "yes" } else { "no" };
            if !m.resolved || m.outcome.as_deref() != Some(side) {
                continue;
            }
            // Already claimed? (receipt exists)
            let event = Pubkey::from_str(&leg.event)?;
            let receipt = vault::win_claim(&event, &a.vault);
            if state.solana.accounts_data(&[receipt]).await?[0].is_some() {
                continue;
            }
            let ix = vault::claim_leg_ix(&a, i as u8, leg, &usdc)?;
            let built = state
                .solana
                .assemble(&crank.pubkey(), vec![vec![ix]], None, CLAIM_CU)
                .await?;
            state.solana.validate(&built.tx, &crank.pubkey(), None)?;
            let signed = VersionedTransaction::try_new(built.tx.message.clone(), &[&crank])?;
            match state.solana.send(&signed).await {
                Ok(sig) => {
                    claimed += 1;
                    tracing::info!(basket = %basket.slug, leg = i, %sig, "vault_crank: claimed leg");
                }
                Err(e) => {
                    tracing::warn!(basket = %basket.slug, leg = i, error = %e, "vault_crank: claim failed")
                }
            }
        }
    }
    if claimed > 0 {
        tracing::info!(claimed, "vault_crank: tick");
    }

    // Permissionless settle for baskets whose markets all resolved long enough ago.
    let rows = client
        .query(
            "SELECT template_id FROM vault_baskets WHERE status IN ('live', 'closed')",
            &[],
        )
        .await?;
    let now = chrono::Utc::now().timestamp();
    for r in rows {
        let Some(basket) = queries::vault::get_basket(&client, r.get(0)).await? else {
            continue;
        };
        let legs: Vec<VaultLeg> = serde_json::from_value(basket.legs_json.clone())?;
        let resolved = vault::resolutions(&state.solana, &legs).await?;
        let Some(last) = resolved
            .iter()
            .copied()
            .collect::<Option<Vec<_>>>()
            .and_then(|v| v.into_iter().max())
        else {
            continue; // some leg still open
        };
        if now < last + vault::SETTLE_GRACE_SECS {
            continue;
        }
        let a = vault::addresses(&program, basket.onchain_id as u64, &usdc);
        let built = state
            .solana
            .assemble(
                &crank.pubkey(),
                vec![vec![vault::settle_after_grace_ix(&a, &legs)?]],
                None,
                CLAIM_CU,
            )
            .await?;
        state.solana.validate(&built.tx, &crank.pubkey(), None)?;
        let signed = VersionedTransaction::try_new(built.tx.message.clone(), &[&crank])?;
        let sig = state.solana.send(&signed).await?;
        if state
            .solana
            .wait_for(&sig, std::time::Duration::from_secs(30))
            .await
            == crate::solana_tx::SigStatus::Confirmed
        {
            queries::vault::set_basket_status(&client, basket.template_id, "settled").await?;
            tracing::info!(basket = %basket.slug, %sig, "vault_crank: settled");
        }
    }
    Ok(())
}
