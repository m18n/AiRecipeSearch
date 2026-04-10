use actix_web::{HttpResponse, ResponseError};
use derive_more::Display;
use serde_json::json;

#[derive(Debug, Display)]
pub enum AppError {
    #[display("Unauthorized: {_0}")]
    Unauthorized(String),

    #[display("Forbidden: {_0}")]
    Forbidden(String),

    #[display("Not found: {_0}")]
    NotFound(String),

    #[display("Bad request: {_0}")]
    BadRequest(String),

    #[display("Unprocessable entity: {_0}")]
    UnprocessableEntity(String),

    #[display("Internal server error: {_0}")]
    InternalServerError(String),

    #[display("Rate limit exceeded from {message}, retry after {retry_after_minutes} minutes")]
    RateLimitError {
        message: String,
        retry_after_minutes: u32,
    },
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::Unauthorized(msg) => HttpResponse::Unauthorized().json(json!({
                "error": "unauthorized",
                "message": msg
            })),

            AppError::Forbidden(msg) => HttpResponse::Forbidden().json(json!({
                "error": "forbidden",
                "message": msg
            })),

            AppError::NotFound(msg) => HttpResponse::NotFound().json(json!({
                "error": "not_found",
                "message": msg
            })),

            AppError::BadRequest(msg) => HttpResponse::BadRequest().json(json!({
                "error": "bad_request",
                "message": msg
            })),

            AppError::UnprocessableEntity(msg) => {
                HttpResponse::UnprocessableEntity().json(json!({
                    "error": "unprocessable_entity",
                    "message": msg
                }))
            }

            AppError::InternalServerError(msg) => {
                HttpResponse::InternalServerError().json(json!({
                    "error": "internal_server_error",
                    "message": msg
                }))
            }

            AppError::RateLimitError {
                message,
                retry_after_minutes,
            } => HttpResponse::TooManyRequests().json(json!({
                "error": "rate_limit",
                "message": message,
                "retry_after_minutes": retry_after_minutes
            })),
        }
    }
}



impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound("Record not found".to_string()),
            _ => AppError::InternalServerError(format!("Database error: {e}")),
        }
    }
}
impl From<actix_web::error::JsonPayloadError> for AppError {
    fn from(e: actix_web::error::JsonPayloadError) -> Self {
        AppError::BadRequest(format!("Invalid JSON payload: {e}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::InternalServerError(format!("HTTP client error: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::InternalServerError(format!("JSON serialization error: {e}"))
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        use jsonwebtoken::errors::ErrorKind;
        match e.kind() {
            ErrorKind::ExpiredSignature => {
                AppError::Unauthorized("Token has expired".to_string())
            }
            ErrorKind::InvalidToken
            | ErrorKind::InvalidSignature
            | ErrorKind::InvalidAlgorithmName => {
                AppError::Unauthorized("Invalid token".to_string())
            }
            _ => AppError::Unauthorized(format!("Token error: {e}")),
        }
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::InternalServerError(format!("Password hashing error: {e}"))
    }
}



/// Parses a `Retry-After` header value (in seconds) into whole minutes, rounded up.
/// Falls back to `default_minutes` if the header is absent or unparseable.
pub fn parse_retry_after_minutes(
    retry_after_header: Option<&str>,
    default_minutes: u32,
) -> u32 {
    retry_after_header
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|seconds| ((seconds + 59) / 60) as u32)
        .unwrap_or(default_minutes)
}