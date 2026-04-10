use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Config;
use crate::db::users;
use crate::error::AppError;





#[derive(Debug, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: i32,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: i32,
    pub exp: usize,
    pub iat: usize,
    pub jti: String,
}





pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}





/// Verify a plain-text password against a bcrypt hash.
pub fn verify_password(plain: &str, hash: &str) -> Result<bool, AppError> {
    bcrypt::verify(plain, hash).map_err(|e| {
        tracing::error!("bcrypt verify error: {e}");
        AppError::InternalServerError("Password verification failed".into())
    })
}

/// Hash a plain-text password with bcrypt (cost = 12).
/// Kept here so that seeding / user-creation utilities can call it.
pub fn hash_password(plain: &str) -> Result<String, AppError> {
    bcrypt::hash(plain, 12).map_err(|e| {
        tracing::error!("bcrypt hash error: {e}");
        AppError::InternalServerError("Password hashing failed".into())
    })
}





/// Generate an Access Token + Refresh Token pair for the given `user_id`.
/// The Refresh Token `jti` is persisted in the `refresh_tokens` table.
pub async fn generate_token_pair(
    user_id: i32,
    pool: &sqlx::PgPool,
    cfg: &Config,
) -> Result<TokenPair, AppError> {
    let now = Utc::now();
    let iat = now.timestamp() as usize;
    let access_exp = (now + cfg.jwt_access_expiration).timestamp() as usize;
    let access_claims = AccessClaims {
        sub: user_id,
        exp: access_exp,
        iat,
    };
    let access_token = encode(
        &Header::new(Algorithm::HS256),
        &access_claims,
        &EncodingKey::from_secret(cfg.jwt_access_secret.as_bytes()),
    )
        .map_err(|e| {
            tracing::error!("Access token encoding error: {e}");
            AppError::InternalServerError("Token generation failed".into())
        })?;
    let jti = Uuid::new_v4().to_string();
    let refresh_exp = now + cfg.jwt_refresh_expiration;
    let refresh_exp_ts = refresh_exp.timestamp() as usize;

    let refresh_claims = RefreshClaims {
        sub: user_id,
        exp: refresh_exp_ts,
        iat,
        jti: jti.clone(),
    };
    let refresh_token = encode(
        &Header::new(Algorithm::HS256),
        &refresh_claims,
        &EncodingKey::from_secret(cfg.jwt_refresh_secret.as_bytes()),
    )
        .map_err(|e| {
            tracing::error!("Refresh token encoding error: {e}");
            AppError::InternalServerError("Token generation failed".into())
        })?;
    users::save_refresh_token(pool, user_id, &jti, refresh_exp).await?;

    Ok(TokenPair {
        access_token,
        refresh_token,
    })
}





/// Validate an Access Token.
/// Returns the decoded `AccessClaims` on success.
pub fn validate_access_token(token: &str, cfg: &Config) -> Result<AccessClaims, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_access_secret.as_bytes()),
        &validation,
    )
        .map(|data| data.claims)
        .map_err(|e| {
            tracing::warn!("Access token validation failed: {e}");
            AppError::Unauthorized("Invalid or expired access token".into())
        })
}

/// Validate a Refresh Token.
///
/// Checks:
/// 1. Signature & expiration (JWT-level).
/// 2. `jti` is present in the `refresh_tokens` table (not revoked).
///
/// Returns the decoded `RefreshClaims` on success.
pub async fn validate_refresh_token(
    token: &str,
    pool: &sqlx::PgPool,
    cfg: &Config,
) -> Result<RefreshClaims, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let claims = decode::<RefreshClaims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_refresh_secret.as_bytes()),
        &validation,
    )
        .map(|data| data.claims)
        .map_err(|e| {
            tracing::warn!("Refresh token JWT validation failed: {e}");
            AppError::Unauthorized("Invalid or expired refresh token".into())
        })?;
    let stored = users::find_refresh_token(pool, &claims.jti).await?;
    if stored.is_none() {
        tracing::warn!("Refresh token jti not found in DB — already revoked");
        return Err(AppError::Unauthorized("Refresh token has been revoked".into()));
    }

    Ok(claims)
}





/// Permanently revoke a Refresh Token by deleting its `jti` from the DB.
/// Called both during **logout** and during **rotation** (the old token is
/// deleted before a new pair is issued).
pub async fn invalidate_refresh_token(jti: &str, pool: &sqlx::PgPool) -> Result<(), AppError> {
    users::delete_refresh_token(pool, jti).await.map(|_| ())
}
pub fn decode_refresh_token_claims(token: &str, cfg: &Config) -> Result<RefreshClaims, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false;

    decode::<RefreshClaims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_refresh_secret.as_bytes()),
        &validation,
    )
        .map(|data| data.claims)
        .map_err(|e| {
            tracing::warn!("Refresh token decode (logout) failed: {e}");
            AppError::Unauthorized("Invalid refresh token".into())
        })
}


pub fn generate_secure_token() -> String {
    let mut bytes = [0u8; 64];
    rand::thread_rng().fill(&mut bytes);
    hex::encode(bytes)
}