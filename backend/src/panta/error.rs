//! Errors from the Panta API, mapped to stable codes (docs/panta-notes.md).

/// Everything that can go wrong when talking to Panta.
#[derive(Debug, thiserror::Error)]
pub enum PantaError {
    /// Panta answered with a documented error envelope
    /// (`{ "code": "…", "message": "…" }`). The code is passed through
    /// unchanged (e.g. `QUOTE_STALE`, `TX_MISMATCH`).
    #[error("panta error {code}: {message}")]
    Api {
        code: String,
        message: String,
        status: u16,
    },

    /// Panta kept answering 429 (or we exceeded our own rate limiter budget)
    /// even after retrying with backoff.
    #[error("rate limited by panta; retry after {retry_after_secs:?}s")]
    RateLimited { retry_after_secs: Option<u64> },

    /// Request timed out (no answer within the client timeout).
    #[error("panta request timed out")]
    Timeout,

    /// Connection / transport failure (DNS, TLS, reset, …).
    #[error("network error talking to panta: {0}")]
    Network(String),

    /// Panta answered 2xx but the body did not match the documented shape.
    #[error("could not decode panta response: {0}")]
    Decode(String),

    /// We caught ourselves sending something invalid before it left the app
    /// (bad wallet, empty market id, …).
    #[error("invalid argument: {0}")]
    Invalid(String),
}

impl PantaError {
    /// Convenience constructor for the pass-through API error case.
    pub fn api(code: &str, message: &str, status: u16) -> Self {
        PantaError::Api {
            code: code.to_string(),
            message: message.to_string(),
            status,
        }
    }

    /// Machine-readable code for callers that need to branch on the failure.
    pub fn code(&self) -> &str {
        match self {
            PantaError::Api { code, .. } => code,
            PantaError::RateLimited { .. } => "RATE_LIMITED",
            PantaError::Timeout => "TIMEOUT",
            PantaError::Network(_) => "NETWORK",
            PantaError::Decode(_) => "DECODE",
            PantaError::Invalid(_) => "INVALID",
        }
    }
}
