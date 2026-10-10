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

    println!("\nSMOKE TEST PASSED");
    Ok(())
}
