use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// JWT claims embedded in each token.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user id).
    pub sub: Uuid,
    /// Expiration time (as UTC timestamp).
    pub exp: usize,
    /// Issued at (as UTC timestamp).
    pub iat: usize,
}

/// Create a JWT token for the given user id.
///
/// # Errors
/// Returns `AppError::Internal` if token encoding fails.
pub fn create_token(user_id: Uuid, secret: &str) -> AppResult<String> {
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);

    let claims = Claims {
        sub: user_id,
        exp: expires_at.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to create token: {e}")))
}

/// Validate a JWT token and return the claims.
///
/// # Errors
/// Returns `AppError::Auth` if the token is invalid or expired.
pub fn validate_token(token: &str, secret: &str) -> AppResult<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Auth(format!("Invalid token: {e}")))
}
