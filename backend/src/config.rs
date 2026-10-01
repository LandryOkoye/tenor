//! Central configuration: every environment variable we read, parsed once at
//! startup. Failing fast here beats failing on the first request.

/// All settings for the backend. Loaded from env vars; see `.env.example`.
#[derive(Debug, Clone)]
pub struct Config {
    /// development | staging | production
    pub app_env: String,
    /// postgres://user:pass@host:port/db
    pub database_url: String,

    // --- Panta (server-side only; never shipped to the browser) ---
    pub panta_api_base_url: String,
    pub panta_api_key: String,
    /// When true, serve recorded fixtures instead of calling Panta.
    /// Refused in production (see `Config::validate`).
    pub panta_mock: bool,

    // --- Solana ---
    pub solana_rpc_url: String,
    pub solana_ws_url: Option<String>,
    /// SPL mint of the settlement currency (mainnet USDC by default).
    pub usdc_mint: String,

    // --- Fees & limits ---
    pub fee_wallet_pubkey: String,
    pub app_fee_bps: u64,
    pub max_basket_legs: usize,
    pub default_slippage_bps: u32,
    /// Priority fee (micro-lamports per compute unit) for every purchase tx.
    pub priority_fee_microlamports: u64,
    /// Panta program ids allowed inside user transactions (safety allowlist).
    /// Required in production; in dev, mock instructions use the System program.
    pub panta_program_ids: Vec<String>,

    // --- Auth / security ---
    pub jwt_secret: String,
    pub admin_wallets: Vec<String>,
    pub blocked_countries: Vec<String>,
    pub cors_origins: Vec<String>,

    /// Address:port the API binds to.
    pub api_bind: String,
}

/// Read one required env var or fail with a clear message.
fn required(key: &str) -> anyhow::Result<String> {
    std::env::var(key).map_err(|_| anyhow::anyhow!("missing required env var {key}"))
}

/// Read an optional env var, falling back to `default`.
fn optional(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Split a comma-separated env var into trimmed entries.
fn list(key: &str) -> Vec<String> {
    std::env::var(key)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let cfg = Config {
            app_env: optional("APP_ENV", "development"),
            database_url: required("DATABASE_URL")?,

            panta_api_base_url: optional(
                "PANTA_API_BASE_URL",
                "https://live-api.panta.market/api/v1",
            ),
            panta_api_key: optional("PANTA_API_KEY", ""),
            panta_mock: optional("PANTA_MOCK", "true").eq_ignore_ascii_case("true"),

            solana_rpc_url: optional("SOLANA_RPC_URL", "https://api.mainnet-beta.solana.com"),
            solana_ws_url: std::env::var("SOLANA_WS_URL").ok(),
            usdc_mint: optional("USDC_MINT", "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),

            fee_wallet_pubkey: optional("FEE_WALLET_PUBKEY", ""),
            app_fee_bps: optional("APP_FEE_BPS", "75").parse()?,
            max_basket_legs: optional("MAX_BASKET_LEGS", "4").parse()?,
            default_slippage_bps: optional("DEFAULT_SLIPPAGE_BPS", "150").parse()?,
            priority_fee_microlamports: optional("PRIORITY_FEE_MICROLAMPORTS", "5000").parse()?,
            panta_program_ids: list("PANTA_PROGRAM_IDS"),

            jwt_secret: optional("JWT_SECRET", "dev-only-insecure-secret"),
            admin_wallets: list("ADMIN_WALLETS"),
            blocked_countries: list("BLOCKED_COUNTRIES"),
            cors_origins: list("CORS_ORIGINS"),

            api_bind: optional("API_BIND", "0.0.0.0:8080"),
        };
        cfg.validate()?;
        Ok(cfg)
    }

    /// Safety rails: never run a production deployment on mock data or with the
    /// default JWT secret (brief §15).
    fn validate(&self) -> anyhow::Result<()> {
        if self.app_env == "production" {
            anyhow::ensure!(
                !self.panta_mock,
                "PANTA_MOCK=true is not allowed in production"
            );
            anyhow::ensure!(
                self.jwt_secret != "dev-only-insecure-secret",
                "JWT_SECRET must be set in production"
            );
            anyhow::ensure!(
                !self.panta_api_key.is_empty(),
                "PANTA_API_KEY must be set in production"
            );
            anyhow::ensure!(
                !self.panta_program_ids.is_empty(),
                "PANTA_PROGRAM_IDS must be set in production (transaction safety allowlist)"
            );
        }
        Ok(())
    }

    pub fn is_production(&self) -> bool {
        self.app_env == "production"
    }
}
