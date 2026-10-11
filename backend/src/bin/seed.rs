//! Seed curated, published baskets straight into the database (no HTTP, no
//! admin wallet needed). Idempotent: existing slugs are skipped.
//!
//! Run once after deploying a demo (mock Panta) environment:
//!   `seed` (same env vars as the api: DATABASE_URL, PANTA_MOCK, …)
//!
//! Baskets reference the mock catalog's market ids, so this is for
//! PANTA_MOCK=true environments; with real Panta, curate via the admin API.

use std::sync::Arc;

use backend::config::Config;
use backend::domain::Side;
use backend::queries;
use backend::state::AppState;

type Leg = (&'static str, Side, i32);

const BASKETS: &[(&str, &str, &str, &str, &[Leg])] = &[
    (
        "afcon-2027-super-eagles",
        "AFCON 2027: Super Eagles",
        "Back Nigeria's AFCON 2027 run. Each leg pays out on its own, so topping the group still pays even if the trophy slips away.",
        "sports",
        &[("mkt_afcon_nigeria_wins_2027", Side::Yes, 5_000), ("mkt_afcon_nigeria_tops_group", Side::Yes, 5_000)],
    ),
    (
        "crypto-bull-2027",
        "Crypto Bull 2027",
        "A view on the next leg up: ETH above $5k and BTC above $120k. Partly right still pays.",
        "crypto",
        &[("mkt_eth_above_5k", Side::Yes, 5_000), ("mkt_btc_above_120k", Side::Yes, 5_000)],
    ),
    (
        "naija-macro-mix",
        "Naija Macro Mix",
        "Naira weakness, a strong Super Eagles year and a crypto rally in one diversified basket.",
        "finance",
        &[
            ("mkt_naira_below_1500", Side::Yes, 3_334),
            ("mkt_afcon_nigeria_tops_group", Side::Yes, 3_333),
            ("mkt_btc_above_120k", Side::Yes, 3_333),
        ],
    ),
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    let pool = backend::db::connect(&config.database_url)?;
    backend::db::migrate(&pool).await?;
    let state = AppState {
        db: pool,
        config: Arc::new(config.clone()),
        panta: backend::panta::make_client(&config),
        solana: Arc::new(backend::solana_tx::SolanaCtx::from_config(&config)?),
    };

    for (slug, title, description, category, legs) in BASKETS {
        let mut client = state.db.get().await?;
        if queries::baskets::get_template_by_slug(&client, slug)
            .await?
            .is_some()
        {
            println!("[skip] {slug}");
            continue;
        }
        // Legs reference the catalog: make sure each market row exists.
        for (market_id, _, _) in legs.iter() {
            backend::routes::markets::ensure_market(&state, market_id)
                .await
                .map_err(|e| anyhow::anyhow!("market {market_id}: {}", e.message))?;
        }
        let id = queries::baskets::create_template(
            &client,
            slug,
            title,
            description,
            Some(category),
            None,
            1_000_000,
            100_000_000,
            "seed",
        )
        .await?;
        let legs: Vec<(String, Side, i32)> = legs
            .iter()
            .map(|(m, s, w)| (m.to_string(), *s, *w))
            .collect();
        queries::baskets::set_legs(&mut client, id, &legs).await?;
        queries::baskets::set_template_status(&client, id, "published").await?;
        println!("[ok]   {slug}");
    }
    Ok(())
}
