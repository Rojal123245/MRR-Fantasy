use axum::{
    extract::Request,
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::error::AppError;

use super::jwt::validate_token;

/// Extension type inserted into requests after successful auth.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
}

/// Axum middleware that validates the JWT bearer token.
///
/// Extracts the `Authorization: Bearer <token>` header, validates it,
/// and injects `AuthUser` into request extensions.
pub async fn auth_middleware(
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let jwt_secret = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default();

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Auth("Missing authorization header".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Auth("Invalid authorization format".to_string()))?;

    let claims = validate_token(token, &jwt_secret)?;

    req.extensions_mut().insert(AuthUser {
        user_id: claims.sub,
    });

    Ok(next.run(req).await)
}
