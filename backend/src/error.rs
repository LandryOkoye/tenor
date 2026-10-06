//! The single error type every handler returns.
//! Serializes as `{ "error": { "code": "...", "message": "...", "details": ... } }`
//! so the frontend can switch on `code` (brief §9).

use crate::domain::ApiError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

/// Application error with a stable machine-readable `code`.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct AppError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl AppError {
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.to_string(),
            message: message.into(),
            details: None,
        }
    }

    pub fn bad_request(code: &str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", message)
    }

    /// Attach structured details to the error body.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Convert into the JSON envelope with the right HTTP status.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ApiError {
            error: crate::domain::ApiErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

/// Let handlers use `?` on database errors without leaking internals.
impl From<tokio_postgres::Error> for AppError {
    fn from(e: tokio_postgres::Error) -> Self {
        tracing::error!(error = ?e, "database error");
        AppError::internal("database error")
    }
}

impl From<deadpool_postgres::PoolError> for AppError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        tracing::error!(error = ?e, "database pool error");
        AppError::internal("database unavailable")
    }
}

/// Map Panta failures onto our error envelope.
///
/// - Documented Panta error codes (e.g. `QUOTE_STALE`, `MARKET_NOT_FOUND`)
///   pass through unchanged with the same HTTP status Panta used, so the
///   frontend can branch on them (brief §9).
/// - Transport problems become `PANTA_UNAVAILABLE` (502) — we never leak
///   internal details to clients.
impl From<crate::panta::PantaError> for AppError {
    fn from(e: crate::panta::PantaError) -> Self {
        match e {
            crate::panta::PantaError::Api {
                code,
                message,
                status,
            } => {
                let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
                AppError::new(status, &code, message)
            }
            crate::panta::PantaError::RateLimited { .. } => AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "prediction market provider rate limit reached; retry shortly",
            ),
            crate::panta::PantaError::Timeout
            | crate::panta::PantaError::Network(_)
            | crate::panta::PantaError::Decode(_) => {
                tracing::error!(error = %e, "panta unavailable");
                AppError::new(
                    StatusCode::BAD_GATEWAY,
                    "PANTA_UNAVAILABLE",
                    "prediction market provider is unreachable",
                )
            }
            crate::panta::PantaError::Invalid(m) => AppError::bad_request("INVALID_REQUEST", m),
        }
    }
}
