use sqlx::PgPool;
use sqlx::types::chrono::{DateTime, Utc};
use crate::error::AppError;

pub struct UserRow {
    pub id: i32,
    pub name: String,
    pub password: String,
}

pub struct RefreshTokenRow {
    pub id: i32,
    pub jti: String,
    pub user_id: i32,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
pub struct PasswordInitTokenRow {
    pub id: i32,
    pub user_id: i32,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn find_user_by_id(
    pool: &PgPool,
    user_id: i32,
) -> Result<Option<UserRow>, AppError> {
    let row = sqlx::query_as!(
        UserRow,
        r#"SELECT id, name, password FROM users WHERE id = $1"#,
        user_id
    )
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(row)
}

pub async fn create_password_init_token(
    pool: &PgPool,
    user_id: i32,
    token: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        DELETE FROM password_init_tokens
        WHERE user_id = $1 AND used_at IS NULL
        "#,
        user_id
    )
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query!(
        r#"
        INSERT INTO password_init_tokens (user_id, token, expires_at)
        VALUES ($1, $2, $3)
        "#,
        user_id,
        token,
        expires_at
    )
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(())
}

pub async fn find_password_init_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<PasswordInitTokenRow>, AppError> {
    let row = sqlx::query_as!(
        PasswordInitTokenRow,
        r#"
        SELECT id, user_id, token, expires_at, used_at, created_at
        FROM password_init_tokens
        WHERE token = $1
        "#,
        token
    )
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(row)
}

pub async fn mark_password_init_token_used(
    pool: &PgPool,
    token: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE password_init_tokens
        SET used_at = NOW()
        WHERE token = $1
        "#,
        token
    )
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(())
}

pub async fn update_user_password(
    pool: &PgPool,
    user_id: i32,
    password_hash: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE users SET password = $1 WHERE id = $2"#,
        password_hash,
        user_id
    )
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(())
}
pub async fn find_user_by_login(
    pool: &PgPool,
    login: &str,
) -> Result<Option<UserRow>, AppError> {
    let row = sqlx::query_as!(
        UserRow,
        r#"
        SELECT id, name, password
        FROM users
        WHERE name = $1
        "#,
        login
    )
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(row)
}

pub async fn save_refresh_token(
    pool: &PgPool,
    user_id: i32,
    jti: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO refresh_tokens (jti, user_id, expires_at)
        VALUES ($1, $2, $3)
        "#,
        jti,
        user_id,
        expires_at
    )
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(())
}

pub async fn find_refresh_token(
    pool: &PgPool,
    jti: &str,
) -> Result<Option<RefreshTokenRow>, AppError> {
    let row = sqlx::query_as!(
        RefreshTokenRow,
        r#"
        SELECT id, jti, user_id, expires_at, created_at
        FROM refresh_tokens
        WHERE jti = $1
        "#,
        jti
    )
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(row)
}

pub async fn delete_refresh_token(
    pool: &PgPool,
    jti: &str,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM refresh_tokens
        WHERE jti = $1
        "#,
        jti
    )
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(result.rows_affected() > 0)
}


pub async fn delete_all_refresh_tokens_for_user(
    pool: &PgPool,
    user_id: i32,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"DELETE FROM refresh_tokens WHERE user_id = $1"#,
        user_id
    )
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(())
}