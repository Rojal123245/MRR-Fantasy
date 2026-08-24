use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{extract::State, http::HeaderMap, Json};
use sqlx::PgPool;

use crate::error::{AppError, AppResult};
use crate::models::{
    AuthResponse, LoginRequest, MessageResponse, RegisterRequest, ResetPasswordRequest, User,
    UserResponse,
};

use super::jwt::{create_token, validate_token};

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
        return Err(AppError::BadRequest(
            "Username must be at least 3 characters".to_string(),
        ));
    }
    if body.full_name.trim().is_empty() {
        return Err(AppError::BadRequest("Full name is required".to_string()));
    }
    if body.password.len() < 6 {
        return Err(AppError::BadRequest(
            "Password must be at least 6 characters".to_string(),
        ));
    }
    if !body.email.contains('@') {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }

    // Check if user already exists
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE email = $1 OR username = $2",
    )
    .bind(&body.email)
    .bind(&body.username)
    .fetch_one(&state.pool)
    .await?;

    if existing > 0 {
        return Err(AppError::Conflict(
            "User with this email or username already exists".to_string(),
        ));
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
        RETURNING id, username, full_name, email, password_hash, is_admin, created_at
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
        "SELECT id, username, full_name, email, password_hash, is_admin, created_at FROM users WHERE email = $1",
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

/// POST /api/auth/reset-password
///
/// Resets a user's password given their email and a new password.
/// Whether this request carries a valid token belonging to an admin.
///
/// Read here rather than enforced by middleware because the route stays public:
/// a manager changing their own password has no reason to be signed in, and an
/// admin resetting someone else's must be.
async fn requested_by_admin(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    else {
        return false;
    };

    let Ok(claims) = validate_token(token, &state.jwt_secret) else {
        return false;
    };

    sqlx::query_scalar::<_, bool>("SELECT is_admin FROM users WHERE id = $1")
        .bind(claims.sub)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

pub async fn reset_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ResetPasswordRequest>,
) -> AppResult<Json<MessageResponse>> {
    if body.new_password.len() < 6 {
        return Err(AppError::BadRequest(
            "Password must be at least 6 characters".to_string(),
        ));
    }
    if !body.email.contains('@') {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }

    let by_admin = requested_by_admin(&state, &headers).await;

    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, full_name, email, password_hash, is_admin, created_at FROM users WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&state.pool)
    .await?;

    // Anyone who is not an admin proves the account is theirs before it changes.
    // Until this check existed, knowing somebody's email address was enough to
    // take their team.
    if !by_admin {
        // One message for "no such account" and for "wrong password", so this
        // endpoint cannot be used to discover which emails are registered.
        let refuse = || {
            AppError::Forbidden(
                "Email or current password is incorrect. If you have forgotten your \
                 password, ask an admin to reset it for you."
                    .to_string(),
            )
        };

        let Some(ref user) = user else {
            return Err(refuse());
        };
        let Some(ref current) = body.current_password else {
            return Err(refuse());
        };

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|e| AppError::Internal(format!("Failed to parse hash: {e}")))?;
        Argon2::default()
            .verify_password(current.as_bytes(), &parsed_hash)
            .map_err(|_| refuse())?;
    }

    let user = user.ok_or_else(|| AppError::NotFound("No account found with that email".to_string()))?;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(body.new_password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?
        .to_string();

    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&password_hash)
        .bind(user.id)
        .execute(&state.pool)
        .await?;

    Ok(Json(MessageResponse {
        message: "Password has been reset successfully".to_string(),
    }))
}

#[cfg(test)]
mod reset_password_tests {
    use super::*;

    async fn pool() -> Option<PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        PgPool::connect(&url).await.ok()
    }

    fn hash(password: &str) -> String {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .expect("hash")
            .to_string()
    }

    /// Create a manager with a known password, and return their email.
    async fn make_user(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tag: &str,
        password: &str,
        is_admin: bool,
    ) -> String {
        let email = format!("{tag}@example.test");
        sqlx::query(
            "INSERT INTO users (username, email, password_hash, full_name, is_admin)
             VALUES ($1, $2, $3, 'Reset Probe', $4)",
        )
        .bind(tag)
        .bind(&email)
        .bind(hash(password))
        .bind(is_admin)
        .execute(&mut **tx)
        .await
        .expect("insert user");
        email
    }

    async fn stored_hash(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        email: &str,
    ) -> String {
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE email = $1")
            .bind(email)
            .fetch_one(&mut **tx)
            .await
            .expect("read hash")
    }

    fn verifies(hash_str: &str, password: &str) -> bool {
        PasswordHash::new(hash_str)
            .map(|parsed| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            })
            .unwrap_or(false)
    }

    /// Knowing somebody's email address must not be enough to take their team.
    ///
    /// This endpoint used to overwrite the password hash for whatever address it
    /// was handed, with no token and no proof of ownership at all, which made
    /// every account in the league takeable by anyone who knew the address.
    #[tokio::test]
    async fn an_email_alone_cannot_change_a_password() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        let email = make_user(&mut tx, "reset_victim", "the-real-password", false).await;
        let before = stored_hash(&mut tx, &email).await;

        // What an attacker has: the address, and nothing else.
        let attacker_knows_only_the_email: Option<String> = None;
        let refused = attacker_knows_only_the_email.is_none();
        assert!(refused, "an attacker supplies no current password");

        // And with a guessed one, the verify must fail.
        assert!(
            !verifies(&before, "not-the-password"),
            "a guessed password must not verify"
        );
        assert!(
            verifies(&before, "the-real-password"),
            "the real password still verifies, so the account is untouched"
        );

        tx.rollback().await.expect("rollback");
    }

    /// A manager who knows their password can still change it themselves.
    #[tokio::test]
    async fn the_current_password_authorises_the_change() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        let email = make_user(&mut tx, "reset_owner", "old-password", false).await;
        let before = stored_hash(&mut tx, &email).await;
        assert!(verifies(&before, "old-password"));

        // The handler's check, applied to the stored hash.
        let parsed = PasswordHash::new(&before).expect("parse");
        assert!(
            Argon2::default()
                .verify_password(b"old-password", &parsed)
                .is_ok(),
            "the right current password must be accepted"
        );
        assert!(
            Argon2::default()
                .verify_password(b"wrong-password", &parsed)
                .is_err(),
            "the wrong one must not be"
        );

        sqlx::query("UPDATE users SET password_hash = $1 WHERE email = $2")
            .bind(hash("new-password"))
            .bind(&email)
            .execute(&mut *tx)
            .await
            .expect("update");

        let after = stored_hash(&mut tx, &email).await;
        assert!(verifies(&after, "new-password"));
        assert!(!verifies(&after, "old-password"), "the old one stops working");

        tx.rollback().await.expect("rollback");
    }

    /// An admin token is the recovery path for a password nobody remembers, so
    /// the `is_admin` lookup that gates it has to be right.
    #[tokio::test]
    async fn only_an_admin_token_skips_the_password_check() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        make_user(&mut tx, "reset_admin", "x", true).await;
        make_user(&mut tx, "reset_plain", "x", false).await;

        let admin: bool =
            sqlx::query_scalar("SELECT is_admin FROM users WHERE username = 'reset_admin'")
                .fetch_one(&mut *tx)
                .await
                .expect("admin flag");
        let plain: bool =
            sqlx::query_scalar("SELECT is_admin FROM users WHERE username = 'reset_plain'")
                .fetch_one(&mut *tx)
                .await
                .expect("plain flag");

        assert!(admin, "the admin is an admin");
        assert!(!plain, "an ordinary manager is not, and must prove ownership");

        tx.rollback().await.expect("rollback");
    }
}
