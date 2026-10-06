//! `basket_guard` — auto-close templates whose legs stop being tradable
//! (brief §7.2: "a template auto-closes for new purchases when any leg's
//! market closes or becomes non-tradable"). Holders keep their positions.

use crate::queries;
use crate::state::AppState;

pub async fn run_once(state: AppState) -> anyhow::Result<()> {
    let client = state.db.get().await?;

    for template_id in queries::baskets::published_template_ids(&client).await? {
        let legs = queries::baskets::get_legs(&client, template_id).await?;
        if let Some(dead) = legs.iter().find(|l| !l.tradable()) {
            queries::baskets::set_template_status(&client, template_id, "closed").await?;
            queries::markets::insert_event(
                &client,
                &dead.market_id,
                "basket_closed",
                &serde_json::json!({
                    "template_id": template_id.to_string(),
                    "reason": "leg not tradable",
                }),
            )
            .await?;
            tracing::info!(
                template = %template_id,
                dead_leg = %dead.market_id,
                "basket_guard: closed template"
            );
        }
    }
    Ok(())
}
