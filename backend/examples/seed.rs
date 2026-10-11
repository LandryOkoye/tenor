//! Seed a dev API (mock Panta) with curated, published baskets so the
//! frontend has real-looking data to build against. Idempotent: existing
//! slugs are skipped.
//!
//! ```bash
//! ./scripts/dev-mock.sh          # starts Postgres + the API in mock mode and seeds
//! # or, against an API you started yourself (dev wallet must be an admin):
//! ADMIN_WALLETS=GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB cargo run --bin api
//! SEED_API=http://localhost:8080 cargo run --example seed
//! ```

use ed25519_dalek::{Signer, SigningKey};
use std::time::Duration;

/// (slug, title, description, category, legs: (market_id, side, weight_bps))
type Basket = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str, i32)],
);

const BASKETS: &[Basket] = &[
    (
        "afcon-2027-super-eagles",
        "AFCON 2027: Super Eagles",
        "Back Nigeria's AFCON 2027 run. Each leg pays out on its own, so topping the group still pays even if the trophy slips away.",
        "sports",
        &[("mkt_afcon_nigeria_wins_2027", "yes", 5_000), ("mkt_afcon_nigeria_tops_group", "yes", 5_000)],
    ),
    (
        "crypto-bull-2027",
        "Crypto Bull 2027",
        "A view on the next leg up: ETH above $5k and BTC above $120k. Partly right still pays.",
        "crypto",
        &[("mkt_eth_above_5k", "yes", 5_000), ("mkt_btc_above_120k", "yes", 5_000)],
    ),
    (
        "naija-macro-mix",
        "Naija Macro Mix",
        "Naira weakness, a strong Super Eagles year and a crypto rally in one diversified basket.",
        "finance",
        &[
            ("mkt_naira_below_1500", "yes", 3_334),
            ("mkt_afcon_nigeria_tops_group", "yes", 3_333),
            ("mkt_btc_above_120k", "yes", 3_333),
        ],
    ),
];

fn api() -> String {
    std::env::var("SEED_API").unwrap_or_else(|_| "http://localhost:8080".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    // Same fixed dev keypair as the smoke test (holds no funds).
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let wallet = bs58::encode(key.verifying_key().as_bytes()).into_string();
    let nonce: serde_json::Value = http
        .post(format!("{}/v1/auth/nonce", api()))
        .json(&serde_json::json!({ "wallet": wallet }))
        .send()
        .await?
        .json()
        .await?;
    let message = nonce["message"].as_str().ok_or("nonce failed")?;
    let signature = bs58::encode(key.sign(message.as_bytes()).to_bytes()).into_string();
    let session: serde_json::Value = http
        .post(format!("{}/v1/auth/verify", api()))
        .json(&serde_json::json!({ "wallet": wallet, "signature": signature }))
        .send()
        .await?
        .json()
        .await?;
    let token = session["token"].as_str().ok_or("login failed")?.to_string();
    // Make sure the catalog is loaded (bootstraps from mock Panta on first call).
    http.get(format!("{}/v1/markets", api())).send().await?;

    for (slug, title, description, category, legs) in BASKETS {
        let exists = http
            .get(format!("{}/v1/baskets/{slug}", api()))
            .send()
            .await?
            .status()
            .is_success();
        if exists {
            println!("  [skip] {slug} already exists");
            continue;
        }
        let created = http
            .post(format!("{}/v1/admin/baskets", api()))
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "slug": slug, "title": title, "description": description,
                "category": category, "cover_url": null,
                "min_buy_usdc": "1000000", "max_buy_usdc": "100000000",
                "legs": legs.iter().map(|(m, s, w)| serde_json::json!({
                    "market_id": m, "side": s, "weight_bps": w
                })).collect::<Vec<_>>(),
            }))
            .send()
            .await?;
        if created.status().as_u16() == 403 {
            return Err(format!("dev wallet {wallet} is not in ADMIN_WALLETS").into());
        }
        let body: serde_json::Value = created.json().await?;
        let id = body["id"]
            .as_str()
            .ok_or_else(|| format!("create {slug} failed: {body}"))?;
        let published = http
            .post(format!("{}/v1/admin/baskets/{id}/publish", api()))
            .bearer_auth(&token)
            .send()
            .await?;
        println!(
            "  [{}] {slug}",
            if published.status().is_success() {
                "ok"
            } else {
                "FAIL"
            }
        );
    }
    Ok(())
}
