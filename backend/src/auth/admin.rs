use axum::{extract::Request, middleware::Next, response::Response};
use sqlx::PgPool;

use crate::error::AppError;

use super::middleware::AuthUser;

/// Axum middleware that ensures the authenticated user has admin privileges.
///
/// Must be applied AFTER `auth_middleware` so that `AuthUser` is available.
pub async fn admin_middleware(req: Request, next: Next) -> Result<Response, AppError> {
    let auth = req
        .extensions()
        .get::<AuthUser>()
        .cloned()
        .ok_or_else(|| AppError::Auth("Not authenticated".to_string()))?;

    let pool = req
        .extensions()
        .get::<PgPool>()
        .cloned()
        .ok_or_else(|| AppError::Internal("Database pool not available".to_string()))?;

    let is_admin: bool = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("DB error: {e}")))?
        .unwrap_or(false);

    if !is_admin {
        return Err(AppError::Auth("Admin access required".to_string()));
    }

    Ok(next.run(req).await)
}
