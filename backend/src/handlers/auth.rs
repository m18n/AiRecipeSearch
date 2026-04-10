use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    db,
    error::AppError,
    services::auth as auth_service,
};
use crate::config::PASSWORD_INIT_TOKEN_TTL_HOURS;
use crate::middleware::auth::AuthenticatedUser;


#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 1, message = "Login must not be empty"))]
    pub login: String,

    #[validate(length(min = 1, message = "Password must not be empty"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RefreshRequest {
    #[validate(length(min = 1, message = "Refresh token must not be empty"))]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LogoutRequest {
    #[validate(length(min = 1, message = "Refresh token must not be empty"))]
    pub refresh_token: String,
}
#[derive(Debug, Deserialize, Validate)]
pub struct SetPasswordRequest {
    #[validate(length(min = 1, message = "token must not be empty"))]
    pub token: String,

    #[validate(length(min = 1, message = "Password must be at least 1 characters"))]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct PasswordInitLinkResponse {
    pub link: String,
    pub expires_at: String,
}


#[derive(Debug, Serialize)]
pub struct TokenPairResponse {
    pub access_token: String,
    pub refresh_token: String,
}



/// POST /api/v1/auth/login
///
/// Accepts `{ login, password }`, verifies credentials against the DB,
/// and returns a fresh token pair on success.
pub async fn login(
    pool: web::Data<sqlx::PgPool>,
    cfg: web::Data<crate::config::Config>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate().map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;
    let user = db::users::find_user_by_login(&pool, &body.login)
        .await?
        .ok_or(AppError::Unauthorized("Invalid login or password".into()))?;
    let password_valid = auth_service::verify_password(&body.password, &user.password)
    .map_err(|_| AppError::Unauthorized("Invalid login or password".into()))?;

    if !password_valid {
        return Err(AppError::Unauthorized("Invalid login or password".into()));
    }
    let token_pair = auth_service::generate_token_pair(user.id,&pool,&cfg).await?;

    Ok(HttpResponse::Ok().json(TokenPairResponse {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
    }))
}

/// POST /api/v1/auth/refresh
///
/// Accepts `{ refresh_token }`, validates it (signature, expiry, jti
/// presence in DB), invalidates the old jti (rotation), and returns
/// a new token pair.
pub async fn refresh(
    pool: web::Data<sqlx::PgPool>,
    cfg: web::Data<crate::config::Config>,
    body: web::Json<RefreshRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate().map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;
    let claims = auth_service::validate_refresh_token(&body.refresh_token,&pool,&cfg).await?;
    auth_service::invalidate_refresh_token( &claims.jti,&pool).await?;
    let token_pair = auth_service::generate_token_pair( claims.sub,&pool,&cfg).await?;

    Ok(HttpResponse::Ok().json(TokenPairResponse {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
    }))
}

/// POST /api/v1/auth/logout  (protected — requires valid Access Token)
///
/// Accepts `{ refresh_token }`, deletes its jti from the DB so it can
/// never be used for rotation again, then returns 204 No Content.
pub async fn logout(
    pool: web::Data<sqlx::PgPool>,
    cfg: web::Data<crate::config::Config>,
    body: web::Json<LogoutRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate().map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    let claims = auth_service::decode_refresh_token_claims(&body.refresh_token, &cfg)
        .map_err(|_| AppError::BadRequest("Invalid refresh token".into()))?;

    auth_service::invalidate_refresh_token(&claims.jti,&pool).await?;

    Ok(HttpResponse::NoContent().finish())
}

/// POST /api/v1/admin/users/{user_id}/password-init-link
///
/// Admin-only. Generates a one-time password initialisation link for any user.
/// The caller must be authenticated AND their `sub` must match `ADMIN_USER_ID`.
pub async fn generate_password_init_link(
    pool: web::Data<sqlx::PgPool>,
    cfg: web::Data<crate::config::Config>,
    path: web::Path<i32>,
    authenticated_user: web::ReqData<AuthenticatedUser>,
) -> Result<HttpResponse, AppError> {
    let target_user_id = path.into_inner();

    if authenticated_user.user_id != cfg.admin_user_id {
        return Err(AppError::Forbidden("Admin access required".into()));
    }
    db::users::find_user_by_id(&pool, target_user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", target_user_id)))?;
    let token = auth_service::generate_secure_token();
    let expires_at = Utc::now()
        + chrono::Duration::hours(PASSWORD_INIT_TOKEN_TTL_HOURS);

    db::users::create_password_init_token(&pool, target_user_id, &token, expires_at).await?;

    let link = format!(
        "{}/set-password?token={}",
        cfg.app_base_url, token
    );

    Ok(HttpResponse::Created().json(PasswordInitLinkResponse {
        link,
        expires_at: expires_at.to_rfc3339(),
    }))
}

/// POST /api/v1/auth/set-password
///
/// Public endpoint. Accepts `{ token, password }`, validates the token,
/// hashes the new password, updates the user, then marks the token as used.
pub async fn set_password(
    pool: web::Data<sqlx::PgPool>,
    body: web::Json<SetPasswordRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;
    let record = db::users::find_password_init_token(&pool, &body.token)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or unknown token".into()))?;
    if record.used_at.is_some() {
        return Err(AppError::BadRequest("Token has already been used".into()));
    }
    if Utc::now() > record.expires_at {
        return Err(AppError::BadRequest("Token has expired".into()));
    }
    let hash = auth_service::hash_password(&body.password)?;
    db::users::update_user_password(&pool, record.user_id, &hash).await?;
    db::users::delete_all_refresh_tokens_for_user(&pool, record.user_id).await?;
    db::users::mark_password_init_token_used(&pool, &body.token).await?;

    Ok(HttpResponse::NoContent().finish())
}