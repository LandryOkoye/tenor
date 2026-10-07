//! End-to-end smoke test for the API (build plan §5).
//!
//! Acts like a headless frontend: wallet-logs-in, browses, buys YES on one
//! market, and reports every step. Run it against a local dev server:
//!
//! ```bash
//! # terminal 1: start the API (mock Panta)
//! DATABASE_URL=postgres://app:app@localhost:5432/app cargo run --bin api
//! # terminal 2:
//! SMOKE_API=http://localhost:8080 cargo run --example smoke
//! ```
//!
//! The wallet is a fixed dev keypair — it never holds funds in mock mode.
//! With real env vars pointed at mainnet, the same script performs the real
//! mainnet smoke buys from the build plan (use a funded test wallet only!).

use base64::Engine;
use ed25519_dalek::{Signer as Ed25519Signer, SigningKey};
use solana_sdk::{
    signer::{keypair::Keypair, Signer},
    transaction::VersionedTransaction,
};
use std::time::Duration;

/// Base URL of the API under test.
fn api_base() -> String {
    std::env::var("SMOKE_API").unwrap_or_else(|_| "http://localhost:8080".to_string())
}

/// Which market to buy on (defaults to a fixture id in mock mode).
fn smoke_market() -> String {
    std::env::var("SMOKE_MARKET").unwrap_or_else(|_| "mkt_afcon_nigeria_wins_2027".to_string())
}

/// How much to spend, in USDC base units (default: 0.50 USDC).
fn smoke_amount() -> String {
    std::env::var("SMOKE_AMOUNT").unwrap_or_else(|_| "500000".to_string())
}

/// One HTTP call helper returning the raw response.
async fn call(
    http: &reqwest::Client,
    method: reqwest::Method,
    path: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    let mut req = http.request(method, format!("{}{}", api_base(), path));
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    if let Some(b) = body {
        req = req.json(&b);
    }
    Ok(req.send().await?)
}

/// Pretty-print a step result; exit on failure.
fn check(name: &str, ok: bool, detail: impl AsRef<str>) {
    if ok {
        println!("  [ok]   {name}: {}", detail.as_ref());
    } else {
        println!("  [FAIL] {name}: {}", detail.as_ref());
        std::process::exit(1);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    println!("SMOKE TEST against {}", api_base());

    // --- 1. Health -----------------------------------------------------------
    let resp = call(&http, reqwest::Method::GET, "/healthz", None, None).await?;
    check(
        "healthz",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );
    let resp = call(&http, reqwest::Method::GET, "/readyz", None, None).await?;
    let body: serde_json::Value = resp.json().await?;
    check("readyz", body["db"] == true, serde_json::to_string(&body)?);

    // --- 2. Wallet login (SIWS) ----------------------------------------------
    // Fixed dev keypair so the run is reproducible.
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    // Fixed dev keypair (deterministic, holds no funds): the 32-byte seed
    // plus its derived public key — the 64-byte form solana Keypair wants.
    let mut keypair_bytes = [0u8; 64];
    keypair_bytes[..32].copy_from_slice(&[7u8; 32]);
    keypair_bytes[32..].copy_from_slice(signing_key.verifying_key().as_bytes());
    let solana_keypair = Keypair::try_from(&keypair_bytes[..])?;
    let wallet = solana_keypair.pubkey().to_string();

    let resp = call(
        &http,
        reqwest::Method::POST,
        "/v1/auth/nonce",
        None,
        Some(serde_json::json!({ "wallet": wallet })),
    )
    .await?;
    check(
        "auth/nonce",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );
    let nonce_resp: serde_json::Value = resp.json().await?;
    let message = nonce_resp["message"].as_str().unwrap().to_string();

    // Sign exactly the message we were given.
    let sig = Ed25519Signer::sign(&signing_key, message.as_bytes());
    let signature_b58 = bs58::encode(sig.to_bytes()).into_string();

    let resp = call(
        &http,
        reqwest::Method::POST,
        "/v1/auth/verify",
        None,
        Some(serde_json::json!({ "wallet": wallet, "signature": signature_b58 })),
    )
    .await?;
    check(
        "auth/verify",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );
    let session: serde_json::Value = resp.json().await?;
    let token = session["token"].as_str().unwrap().to_string();

    // --- 3. Browse markets -----------------------------------------------------
    let resp = call(&http, reqwest::Method::GET, "/v1/markets", None, None).await?;
    let page: serde_json::Value = resp.json().await?;
    let n = page["items"].as_array().map(Vec::len).unwrap_or(0);
    check("markets list", n > 0, format!("{n} markets"));

    let market_id = smoke_market();
    let resp = call(
        &http,
        reqwest::Method::GET,
        &format!("/v1/markets/{market_id}"),
        None,
        None,
    )
    .await?;
    let detail: serde_json::Value = resp.json().await?;
    check(
        "market detail",
        detail["title"].is_string(),
        detail["title"].as_str().unwrap_or("?"),
    );

    // --- 4. Quote -> build -> sign -> submit ----------------------------------
    let resp = call(
        &http,
        reqwest::Method::POST,
        "/v1/orders/quote",
        Some(&token),
        Some(serde_json::json!({
            "market_id": market_id,
            "side": "yes",
            "amount_usdc": smoke_amount(),
        })),
    )
    .await?;
    check(
        "orders/quote",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );
    let quote: serde_json::Value = resp.json().await?;
    let order_id = quote["quote_id"].as_str().unwrap().to_string();
    println!(
        "         quote: {} shares @ {} (panta fee {})",
        quote["expected_shares"], quote["avg_price"], quote["panta_fee_usdc"]
    );

    let resp = call(
        &http,
        reqwest::Method::POST,
        &format!("/v1/orders/{order_id}/build"),
        Some(&token),
        None,
    )
    .await?;
    check(
        "orders/build",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );
    let built: serde_json::Value = resp.json().await?;
    let tx_b64 = built["transactions"][0].as_str().unwrap().to_string();

    // "The wallet": take the unsigned message and produce a signed tx
    // with our dev keypair (exactly what Phantom/Solflare would do).
    let tx_bytes = base64::engine::general_purpose::STANDARD.decode(&tx_b64)?;
    let unsigned: VersionedTransaction = bincode::deserialize(&tx_bytes)?;
    let signed_tx = VersionedTransaction::try_new(unsigned.message, &[&solana_keypair])?;
    let signed_b64 = base64::engine::general_purpose::STANDARD
        .encode(bincode::serialize(&signed_tx).expect("serialize signed tx"));

    let resp = call(
        &http,
        reqwest::Method::POST,
        &format!("/v1/orders/{order_id}/submit"),
        Some(&token),
        Some(serde_json::json!({ "signed_txs": [signed_b64] })),
    )
    .await?;
    check(
        "orders/submit",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );
    let view: serde_json::Value = resp.json().await?;
    println!(
        "         order status: {}, signature: {}",
        view["status"],
        view["tx_signature"].as_str().unwrap_or("-")
    );
    check(
        "order filled",
        view["status"] == "filled",
        format!("status {}", view["status"]),
    );

    // --- 5. Basket: admin curates, member buys in ONE signature -------------
    // The smoke wallet must be on ADMIN_WALLETS for curation; otherwise this
    // section reports how to enable it and skips.
    let slug = format!(
        "smoke-basket-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let create = call(
        &http,
        reqwest::Method::POST,
        "/v1/admin/baskets",
        Some(&token),
        Some(serde_json::json!({
            "slug": slug,
            "title": "Smoke Test Basket",
            "description": "Three-leg basket created by the smoke tool.",
            "category": "sports",
            "cover_url": null,
            "min_buy_usdc": "1000000",
            "max_buy_usdc": "100000000",
            "legs": [
                { "market_id": "mkt_afcon_nigeria_wins_2027", "side": "yes", "weight_bps": 4000 },
                { "market_id": "mkt_eth_above_5k",           "side": "yes", "weight_bps": 3000 },
                { "market_id": "mkt_naira_below_1500",        "side": "yes", "weight_bps": 3000 }
            ]
        })),
    )
    .await?;

    if create.status().as_u16() == 403 {
        println!(
            "  [skip] basket flow: add the smoke wallet to ADMIN_WALLETS to enable:\n         ADMIN_WALLETS={wallet}"
        );
        println!("\nSMOKE TEST PASSED (baskets skipped)");
        return Ok(());
    }
    check(
        "admin create basket",
        create.status().is_success(),
        format!("status {}", create.status()),
    );
    let template: serde_json::Value = create.json().await?;
    let template_id = template["id"].as_str().unwrap().to_string();

    let resp = call(
        &http,
        reqwest::Method::POST,
        &format!("/v1/admin/baskets/{template_id}/publish"),
        Some(&token),
        None,
    )
    .await?;
    check(
        "admin publish basket",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );

    // Member views the basket.
    let resp = call(
        &http,
        reqwest::Method::GET,
        &format!("/v1/baskets/{slug}"),
        None,
        None,
    )
    .await?;
    let detail: serde_json::Value = resp.json().await?;
    check(
        "basket detail",
        detail["legs"].as_array().map(Vec::len).unwrap_or(0) == 3,
        format!(
            "index price {}",
            detail["index_price"].as_str().unwrap_or("-")
        ),
    );

    // Quote 2 USDC.
    let resp = call(
        &http,
        reqwest::Method::POST,
        &format!("/v1/baskets/{template_id}/quote"),
        Some(&token),
        Some(serde_json::json!({ "amount_usdc": "2000000", "slippage_bps": 150 })),
    )
    .await?;
    check(
        "basket quote",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );
    let quote: serde_json::Value = resp.json().await?;
    println!(
        "         basket fee: {} (ours), {} (panta total)",
        quote["our_fee_usdc"], quote["total_panta_fees_usdc"]
    );
    let basket_quote_id = quote["quote_id"].as_str().unwrap().to_string();

    // Build -> ONE transaction for all legs + fee.
    let resp = call(
        &http,
        reqwest::Method::POST,
        &format!("/v1/baskets/quotes/{basket_quote_id}/build"),
        Some(&token),
        None,
    )
    .await?;
    check(
        "basket build",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );
    let built: serde_json::Value = resp.json().await?;
    assert_eq!(
        built["transactions"].as_array().map(Vec::len),
        Some(1),
        "baskets must be one transaction"
    );
    let purchase_id = built["purchase_id"].as_str().unwrap().to_string();
    let tx_b64 = built["transactions"][0].as_str().unwrap().to_string();

    // Sign and submit.
    let tx_bytes = base64::engine::general_purpose::STANDARD.decode(&tx_b64)?;
    let unsigned: VersionedTransaction = bincode::deserialize(&tx_bytes)?;
    let signed_tx = VersionedTransaction::try_new(unsigned.message, &[&solana_keypair])?;
    let signed_b64 = base64::engine::general_purpose::STANDARD
        .encode(bincode::serialize(&signed_tx).expect("serialize signed tx"));

    let resp = call(
        &http,
        reqwest::Method::POST,
        &format!("/v1/baskets/purchases/{purchase_id}/submit"),
        Some(&token),
        Some(serde_json::json!({ "signed_txs": [signed_b64] })),
    )
    .await?;
    check(
        "basket submit",
        resp.status().is_success(),
        format!("status {}", resp.status()),
    );

    // Poll the purchase: every leg must be filled.
    let resp = call(
        &http,
        reqwest::Method::GET,
        &format!("/v1/baskets/purchases/{purchase_id}"),
        Some(&token),
        None,
    )
    .await?;
    let purchase: serde_json::Value = resp.json().await?;
    let filled = purchase["legs"]
        .as_array()
        .map(|legs| legs.iter().all(|l| l["status"] == "filled"))
        .unwrap_or(false);
    check(
        "basket purchase filled",
        purchase["status"] == "filled" && filled,
        format!(
            "status {}, {} legs",
            purchase["status"],
            purchase["legs"].as_array().map(Vec::len).unwrap_or(0)
        ),
    );

    // --- 6. Portfolio + activity --------------------------------------------
    let resp = call(
        &http,
        reqwest::Method::GET,
        "/v1/portfolio",
        Some(&token),
        None,
    )
    .await?;
    let pf: serde_json::Value = resp.json().await?;
    let basket_count = pf["baskets"].as_array().map(Vec::len).unwrap_or(0);
    let total = pf["total_value_usdc"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    check(
        "portfolio",
        basket_count >= 1 && total > 0,
        format!("{basket_count} baskets, total value {total} base"),
    );
    println!(
        "         basket: cost {} value {} pnl {}",
        pf["baskets"][0]["cost_basis_usdc"],
        pf["baskets"][0]["current_value_usdc"],
        pf["baskets"][0]["unrealized_pnl_usdc"],
    );

    let resp = call(
        &http,
        reqwest::Method::GET,
        "/v1/portfolio/activity",
        Some(&token),
        None,
    )
    .await?;
    let act: serde_json::Value = resp.json().await?;
    let n = act.as_array().map(Vec::len).unwrap_or(0);
    check("portfolio activity", n >= 2, format!("{n} items"));

    // --- 7. Resolution Center -------------------------------------------------
    let resp = call(
        &http,
        reqwest::Method::GET,
        "/v1/resolution/markets?status=resolved",
        None,
        None,
    )
    .await?;
    let resolved: serde_json::Value = resp.json().await?;
    let n = resolved.as_array().map(Vec::len).unwrap_or(0);
    check(
        "resolution: resolved list",
        n >= 1,
        format!("{n} resolved markets"),
    );

    let resp = call(
        &http,
        reqwest::Method::GET,
        &format!("/v1/resolution/markets/{market_id}"),
        None,
        None,
    )
    .await?;
    let rdetail: serde_json::Value = resp.json().await?;
    let timeline_len = rdetail["timeline"].as_array().map(Vec::len).unwrap_or(0);
    check(
        "resolution: market timeline",
        timeline_len >= 1,
        format!("{timeline_len} events"),
    );

    // --- 8. Claims: nothing claimable in this run (all buys on open markets)
    // The positive claim path is covered by unit tests; here we assert the
    // honest negative: the API refuses to build claims with nothing to claim.
    let resp = call(
        &http,
        reqwest::Method::POST,
        "/v1/claims/build",
        Some(&token),
        Some(serde_json::json!({ "market_ids": [] })),
    )
    .await?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    check(
        "claims: nothing claimable (all buys on open markets)",
        status.as_u16() == 400 && body["error"]["code"] == "NOTHING_CLAIMABLE",
        format!("status {status} code {}", body["error"]["code"]),
    );

    println!("\nSMOKE TEST PASSED");
    Ok(())
}
