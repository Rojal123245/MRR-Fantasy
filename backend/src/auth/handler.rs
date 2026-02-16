use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{extract::State, Json};
use sqlx::PgPool;

use crate::error::{AppError, AppResult};
use crate::models::{AuthResponse, LoginRequest, RegisterRequest, User, UserResponse};

use super::jwt::create_token;

/// Shared application state passed to handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
}

/// POST /api/auth/register
///
/// Creates a new user account with hashed password.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> AppResult<Json<AuthResponse>> {
    // Validate input
    if body.username.len() < 3 {
        return Err(AppError::BadRequest("Username must be at least 3 characters".to_string()));
    }
    if body.full_name.trim().is_empty() {
        return Err(AppError::BadRequest("Full name is required".to_string()));
    }
    if body.password.len() < 6 {
        return Err(AppError::BadRequest("Password must be at least 6 characters".to_string()));
    }
    if !body.email.contains('@') {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }

    // Check if user already exists
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE email = $1 OR username = $2"
    )
    .bind(&body.email)
    .bind(&body.username)
    .fetch_one(&state.pool)
    .await?;

    if existing > 0 {
        return Err(AppError::Conflict("User with this email or username already exists".to_string()));
    }

    // Hash password
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(body.password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?
        .to_string();

    // Insert user
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, full_name, email, password_hash)
        VALUES ($1, $2, $3, $4)
        RETURNING id, username, full_name, email, password_hash, created_at
        "#,
    )
    .bind(&body.username)
    .bind(body.full_name.trim())
    .bind(&body.email)
    .bind(&password_hash)
    .fetch_one(&state.pool)
    .await?;

    let token = create_token(user.id, &state.jwt_secret)?;
    let user_response: UserResponse = user.into();

    Ok(Json(AuthResponse {
        token,
        user: user_response,
    }))
}

/// POST /api/auth/login
///
/// Authenticates a user and returns a JWT token.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, full_name, email, password_hash, created_at FROM users WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Auth("Invalid email or password".to_string()))?;

    // Verify password
    let parsed_hash = PasswordHash::new(&user.password_hash)
        .map_err(|e| AppError::Internal(format!("Failed to parse hash: {e}")))?;

    Argon2::default()
        .verify_password(body.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Auth("Invalid email or password".to_string()))?;

    let token = create_token(user.id, &state.jwt_secret)?;
    let user_response: UserResponse = user.into();

    Ok(Json(AuthResponse {
        token,
        user: user_response,
    }))
}
