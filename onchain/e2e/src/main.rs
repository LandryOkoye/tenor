//! basket_vault e2e (localnet). Usage:
//!   vault_e2e fixtures <dir>   write account fixtures (funded user USDC ATA)
//!   vault_e2e run              create basket, deposit twice, check NAV minting
use std::str::FromStr;

use base64::Engine;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};

const VAULT_PROGRAM: &str = "AGrL8gCofvFTvsLDXmwThAWd1Wdz9ULPYwHWCzVDnF7u";
const PANTA: &str = "6gM5afTQBq5VZCfgpGqcsqzfWd5maLSCKWtGjbEobZMp";
const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const CONFIG: &str = "8mJjfx7SuWwS4yfVZXtqC2cvzxr3TQHsCgKyjpuDWKAP";
const PANTA_FEE: &str = "9hwZLD2JZuWFDDQXEnHsnny7EF2LeyHqwcMFVssYEGpC";
/// (event, market authority, market vault) of the two legs. Both markets
/// must still be in Panta's primary (flash acquisition) window, so they are
/// passed in by run.sh (E2E_MARKET1 / E2E_MARKET2, "event auth vault").
fn markets() -> [(Pubkey, Pubkey, Pubkey); 2] {
    let leg = |var: &str| {
        let v = std::env::var(var).unwrap_or_else(|_| panic!("{var} not set (see run.sh)"));
        let p: Vec<Pubkey> = v.split_whitespace().map(pk).collect();
        assert_eq!(p.len(), 3, "{var} must be \"event authority vault\"");
        (p[0], p[1], p[2])
    };
    [leg("E2E_MARKET1"), leg("E2E_MARKET2")]
}

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}
fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), pk(TOKEN).as_ref(), mint.as_ref()],
        &pk(ATA),
    )
    .0
}
fn disc(name: &str) -> Vec<u8> {
    hash(format!("global:{name}").as_bytes()).to_bytes()[..8].to_vec()
}
fn dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("keys")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    std::fs::create_dir_all(dir()).unwrap();
    let user = load_or_new("user.json");
    let admin = load_or_new("admin.json");
    match args.get(1).map(String::as_str) {
        Some("fixtures") => {
            let out = std::path::PathBuf::from(&args[2]);
            let mut d = Vec::with_capacity(165);
            d.extend_from_slice(pk(USDC).as_ref());
            d.extend_from_slice(user.pubkey().as_ref());
            d.extend_from_slice(&20_000_000u64.to_le_bytes()); // 20 USDC
            d.extend_from_slice(&[0u8; 36]);
            d.push(1);
            d.extend_from_slice(&[0u8; 12]);
            d.extend_from_slice(&0u64.to_le_bytes());
            d.extend_from_slice(&[0u8; 36]);
            let key = ata(&user.pubkey(), &pk(USDC));
            let j = serde_json::json!({"pubkey": key.to_string(), "account": {
                "lamports": 2_039_280u64, "data": [base64::engine::general_purpose::STANDARD.encode(&d), "base64"],
                "owner": TOKEN, "executable": false, "rentEpoch": 0, "space": 165}});
            std::fs::write(out.join("user_usdc.json"), j.to_string()).unwrap();

            println!("{} {}", admin.pubkey(), user.pubkey());
        }
        Some("run") => run(&admin, &user),
        _ => eprintln!("usage: vault_e2e fixtures <dir> | run"),
    }
}

fn load_or_new(name: &str) -> Keypair {
    let p = dir().join(name);
    read_keypair_file(&p).unwrap_or_else(|_| {
        let k = Keypair::new();
        solana_sdk::signature::write_keypair_file(&k, &p).unwrap();
        k
    })
}

fn send(
    rpc: &RpcClient,
    ixs: &[Instruction],
    payer: &Keypair,
    extra: &[&Keypair],
) -> Result<(), String> {
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        &signers,
        rpc.get_latest_blockhash().unwrap(),
    );
    let sim = rpc.simulate_transaction(&tx).unwrap().value;
    if let Some(err) = sim.err {
        for l in sim.logs.unwrap_or_default() {
            println!("    {l}");
        }
        return Err(format!("{err:?}"));
    }
    rpc.send_and_confirm_transaction(&tx)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn balance(rpc: &RpcClient, acct: &Pubkey) -> u64 {
    rpc.get_token_account_balance(acct)
        .map(|b| b.amount.parse().unwrap())
        .unwrap_or(0)
}

fn run(admin: &Keypair, user: &Keypair) {
    let rpc = RpcClient::new_with_commitment(
        "http://127.0.0.1:8899".to_string(),
        CommitmentConfig::confirmed(),
    );
    let program = pk(VAULT_PROGRAM);
    let usdc = pk(USDC);
    let id: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let basket = Pubkey::find_program_address(&[b"basket", &id.to_le_bytes()], &program).0;
    let vault = Pubkey::find_program_address(&[b"vault", basket.as_ref()], &program).0;
    let mint = Pubkey::find_program_address(&[b"mint", basket.as_ref()], &program).0;
    let vault_usdc = ata(&vault, &usdc);
    let fee_usdc = ata(&admin.pubkey(), &usdc);
    let user_usdc = ata(&user.pubkey(), &usdc);
    let user_basket = ata(&user.pubkey(), &mint);
    rpc.request_airdrop(&user.pubkey(), 2_000_000_000).ok();
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Fee wallet ATA (admin) + create_basket: 50/50, leg 1 YES, leg 2 NO, 75 bps.
    let create_fee_ata = Instruction {
        program_id: pk(ATA),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(fee_usdc, false),
            AccountMeta::new_readonly(admin.pubkey(), false),
            AccountMeta::new_readonly(usdc, false),
            AccountMeta::new_readonly(
                solana_sdk::pubkey!("11111111111111111111111111111111"),
                false,
            ),
            AccountMeta::new_readonly(pk(TOKEN), false),
        ],
        data: vec![1], // CreateIdempotent
    };
    let mut data = disc("create_basket");
    data.extend_from_slice(&id.to_le_bytes());
    data.extend_from_slice(&2u32.to_le_bytes());
    for (i, (event, _, _)) in markets().iter().enumerate() {
        data.extend_from_slice(event.as_ref());
        data.push(i as u8); // side: YES then NO
        data.extend_from_slice(&5_000u16.to_le_bytes());
    }
    data.extend_from_slice(&75u16.to_le_bytes());
    let create = Instruction {
        program_id: program,
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(basket, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(mint, false),
            AccountMeta::new(vault_usdc, false),
            AccountMeta::new_readonly(fee_usdc, false),
            AccountMeta::new_readonly(usdc, false),
            AccountMeta::new_readonly(pk(TOKEN), false),
            AccountMeta::new_readonly(pk(ATA), false),
            AccountMeta::new_readonly(
                solana_sdk::pubkey!("11111111111111111111111111111111"),
                false,
            ),
        ],
        data,
    };
    report(
        "create_basket",
        send(&rpc, &[create_fee_ata, create], admin, &[]),
    );

    // User's basket-token ATA.
    let create_user_basket = Instruction {
        program_id: pk(ATA),
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(user_basket, false),
            AccountMeta::new_readonly(user.pubkey(), false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(
                solana_sdk::pubkey!("11111111111111111111111111111111"),
                false,
            ),
            AccountMeta::new_readonly(pk(TOKEN), false),
        ],
        data: vec![1],
    };
    report(
        "user basket ATA",
        send(&rpc, &[create_user_basket], user, &[]),
    );

    let deposit_ix = |amount: u64, min_out: u64| {
        let mut data = disc("deposit");
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(&min_out.to_le_bytes());
        let mut accounts = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(basket, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(mint, false),
            AccountMeta::new(vault_usdc, false),
            AccountMeta::new(user_usdc, false),
            AccountMeta::new(user_basket, false),
            AccountMeta::new(fee_usdc, false),
            AccountMeta::new_readonly(usdc, false),
            AccountMeta::new_readonly(pk(CONFIG), false),
            AccountMeta::new(pk(PANTA_FEE), false),
            AccountMeta::new_readonly(pk(PANTA), false),
            AccountMeta::new_readonly(pk(TOKEN), false),
            AccountMeta::new_readonly(pk(ATA), false),
            AccountMeta::new_readonly(
                solana_sdk::pubkey!("11111111111111111111111111111111"),
                false,
            ),
        ];
        for (event, auth, mvault) in markets() {
            let position = Pubkey::find_program_address(
                &[b"position", event.as_ref(), vault.as_ref()],
                &pk(PANTA),
            )
            .0;
            accounts.push(AccountMeta::new(event, false));
            accounts.push(AccountMeta::new_readonly(auth, false));
            accounts.push(AccountMeta::new(mvault, false));
            accounts.push(AccountMeta::new(position, false));
        }
        Instruction {
            program_id: program,
            accounts,
            data,
        }
    };
    let cu = || ComputeBudgetInstruction::set_compute_unit_limit(600_000);

    // Deposit 1: 4 USDC -> first deposit mints ~value.
    let u0 = balance(&rpc, &user_usdc);
    report(
        "deposit 4 USDC",
        send(&rpc, &[cu(), deposit_ix(4_000_000, 1)], user, &[]),
    );
    let (t1, u1) = (balance(&rpc, &user_basket), balance(&rpc, &user_usdc));
    println!(
        "  tokens={t1} user_usdc {u0}->{u1} fee_wallet={} vault_cash={}",
        balance(&rpc, &fee_usdc),
        balance(&rpc, &vault_usdc)
    );

    // Slippage guard: absurd min_tokens_out must revert.
    let guard = send(&rpc, &[cu(), deposit_ix(2_000_000, u64::MAX)], user, &[]);
    report(
        "slippage guard rejects (expected error)",
        if guard.is_err() {
            Ok(())
        } else {
            Err("deposit with min=u64::MAX succeeded".into())
        },
    );

    // Deposit 2: 2 USDC -> ~half the tokens of deposit 1 (minus its price impact).
    report(
        "deposit 2 USDC",
        send(&rpc, &[cu(), deposit_ix(2_000_000, 1)], user, &[]),
    );
    let t2 = balance(&rpc, &user_basket) - t1;
    println!(
        "  second mint={t2} ({:.1}% of first; ~50% expected)",
        t2 as f64 * 100.0 / t1 as f64
    );
    let ok = t1 > 3_000_000 && t1 < 4_000_000 && t2 * 100 > t1 * 40 && t2 * 100 < t1 * 55;
    report(
        "NAV minting sane",
        if ok {
            Ok(())
        } else {
            Err(format!("t1={t1} t2={t2}"))
        },
    );
}

fn report(step: &str, r: Result<(), String>) {
    match r {
        Ok(()) => println!("[ok]   {step}"),
        Err(e) => {
            println!("[FAIL] {step}: {e}");
            std::process::exit(1);
        }
    }
}
