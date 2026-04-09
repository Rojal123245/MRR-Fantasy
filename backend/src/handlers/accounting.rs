use axum::{
    extract::{Path, State},
    Extension, Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FutsalSessionRow {
    pub id: Uuid,
    pub title: String,
    pub total_amount: Decimal,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct FutsalSessionResponse {
    pub id: Uuid,
    pub title: String,
    pub total_amount: String,
    pub player_count: i64,
    pub paid_count: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SessionPlayerRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub user_id: Option<Uuid>,
    pub player_name: String,
    pub amount_due: Decimal,
    pub is_paid: bool,
    pub paid_at: Option<chrono::DateTime<chrono::Utc>>,
    pub marked_paid_by: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct SessionPlayerResponse {
    pub id: Uuid,
    pub session_id: Uuid,
    pub user_id: Option<Uuid>,
    pub player_name: String,
    pub amount_due: String,
    pub is_paid: bool,
    pub paid_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionDetailResponse {
    pub session: FutsalSessionResponse,
    pub players: Vec<SessionPlayerResponse>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserListItem {
    pub id: Uuid,
    pub username: String,
    pub full_name: String,
}

#[derive(Debug, Serialize)]
pub struct UserDue {
    pub session_id: Uuid,
    pub session_title: String,
    pub player_entry_id: Uuid,
    pub amount_due: String,
    pub is_paid: bool,
}

#[derive(Debug, Serialize)]
pub struct UserSummaryItem {
    pub user_id: Option<Uuid>,
    pub player_name: String,
    pub total_due: String,
    pub total_paid: String,
    pub total_unpaid: String,
    pub sessions_count: i64,
}

// ── Request types ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub title: String,
    pub total_amount: f64,
}

#[derive(Debug, Deserialize)]
pub struct AddPlayerRequest {
    pub user_id: Option<Uuid>,
    pub player_name: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

async fn recalculate_amounts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: Uuid,
) -> Result<(), sqlx::Error> {
    let total: Decimal =
        sqlx::query_scalar("SELECT total_amount FROM futsal_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&mut **tx)
            .await?;

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM futsal_session_players WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&mut **tx)
            .await?;

    if count == 0 {
        return Ok(());
    }

    let per_player = total / Decimal::from(count);

    sqlx::query(
        "UPDATE futsal_session_players SET amount_due = $1 WHERE session_id = $2",
    )
    .bind(per_player)
    .bind(session_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn format_session(row: FutsalSessionRow, player_count: i64, paid_count: i64) -> FutsalSessionResponse {
    FutsalSessionResponse {
        id: row.id,
        title: row.title,
        total_amount: row.total_amount.to_string(),
        player_count,
        paid_count,
        created_at: row.created_at.to_rfc3339(),
    }
}

fn format_player(row: SessionPlayerRow) -> SessionPlayerResponse {
    SessionPlayerResponse {
        id: row.id,
        session_id: row.session_id,
        user_id: row.user_id,
        player_name: row.player_name,
        amount_due: row.amount_due.to_string(),
        is_paid: row.is_paid,
        paid_at: row.paid_at.map(|t| t.to_rfc3339()),
    }
}

// ── Admin routes ────────────────────────────────────────────────────────────

/// POST /api/accounting/sessions
pub async fn create_session(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateSessionRequest>,
) -> AppResult<Json<FutsalSessionResponse>> {
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("Title is required".into()));
    }
    if body.total_amount <= 0.0 {
        return Err(AppError::BadRequest("Total amount must be positive".into()));
    }

    let amount = Decimal::try_from(body.total_amount)
        .map_err(|_| AppError::BadRequest("Invalid amount".into()))?;

    let row = sqlx::query_as::<_, FutsalSessionRow>(
        r#"INSERT INTO futsal_sessions (title, total_amount, created_by)
           VALUES ($1, $2, $3)
           RETURNING id, title, total_amount, created_by, created_at"#,
    )
    .bind(body.title.trim())
    .bind(amount)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(format_session(row, 0, 0)))
}

/// GET /api/accounting/sessions
pub async fn list_sessions(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FutsalSessionResponse>>> {
    let rows = sqlx::query_as::<_, FutsalSessionRow>(
        "SELECT id, title, total_amount, created_by, created_at FROM futsal_sessions ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let sid = row.id;
        let player_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM futsal_session_players WHERE session_id = $1")
                .bind(sid)
                .fetch_one(&state.pool)
                .await?;
        let paid_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM futsal_session_players WHERE session_id = $1 AND is_paid = true",
        )
        .bind(sid)
        .fetch_one(&state.pool)
        .await?;
        result.push(format_session(row, player_count, paid_count));
    }

    Ok(Json(result))
}

/// GET /api/accounting/sessions/:id
pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<SessionDetailResponse>> {
    let row = sqlx::query_as::<_, FutsalSessionRow>(
        "SELECT id, title, total_amount, created_by, created_at FROM futsal_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".into()))?;

    let players = sqlx::query_as::<_, SessionPlayerRow>(
        r#"SELECT id, session_id, user_id, player_name, amount_due, is_paid, paid_at, marked_paid_by
           FROM futsal_session_players
           WHERE session_id = $1
           ORDER BY is_paid ASC, player_name ASC"#,
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;

    let player_count = players.len() as i64;
    let paid_count = players.iter().filter(|p| p.is_paid).count() as i64;

    Ok(Json(SessionDetailResponse {
        session: format_session(row, player_count, paid_count),
        players: players.into_iter().map(format_player).collect(),
    }))
}

/// DELETE /api/accounting/sessions/:id
pub async fn delete_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let result = sqlx::query("DELETE FROM futsal_sessions WHERE id = $1")
        .bind(session_id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Session not found".into()));
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /api/accounting/sessions/:id/players
pub async fn add_player(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<AddPlayerRequest>,
) -> AppResult<Json<SessionDetailResponse>> {
    if body.player_name.trim().is_empty() {
        return Err(AppError::BadRequest("Player name is required".into()));
    }

    // Verify session exists
    let _session = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM futsal_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".into()))?;

    let mut tx = state.pool.begin().await?;

    sqlx::query(
        r#"INSERT INTO futsal_session_players (session_id, user_id, player_name)
           VALUES ($1, $2, $3)"#,
    )
    .bind(session_id)
    .bind(body.user_id)
    .bind(body.player_name.trim())
    .execute(&mut *tx)
    .await?;

    recalculate_amounts(&mut tx, session_id).await?;

    tx.commit().await?;

    // Return updated session detail
    get_session_inner(&state.pool, session_id).await
}

/// DELETE /api/accounting/sessions/:session_id/players/:player_id
pub async fn remove_player(
    State(state): State<AppState>,
    Path((session_id, player_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<SessionDetailResponse>> {
    let mut tx = state.pool.begin().await?;

    let result = sqlx::query(
        "DELETE FROM futsal_session_players WHERE id = $1 AND session_id = $2",
    )
    .bind(player_id)
    .bind(session_id)
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Player entry not found".into()));
    }

    recalculate_amounts(&mut tx, session_id).await?;

    tx.commit().await?;

    get_session_inner(&state.pool, session_id).await
}

/// PUT /api/accounting/sessions/:session_id/players/:player_id/pay
pub async fn toggle_pay(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((session_id, player_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<SessionDetailResponse>> {
    let entry = sqlx::query_as::<_, SessionPlayerRow>(
        r#"SELECT id, session_id, user_id, player_name, amount_due, is_paid, paid_at, marked_paid_by
           FROM futsal_session_players
           WHERE id = $1 AND session_id = $2"#,
    )
    .bind(player_id)
    .bind(session_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Player entry not found".into()))?;

    // Non-admin users can only toggle their own entries
    let is_admin: bool = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await?
        .unwrap_or(false);

    if !is_admin {
        if entry.user_id != Some(auth.user_id) {
            return Err(AppError::Auth("You can only mark your own dues as paid".into()));
        }
    }

    let new_paid = !entry.is_paid;

    if new_paid {
        sqlx::query(
            "UPDATE futsal_session_players SET is_paid = true, paid_at = NOW(), marked_paid_by = $1 WHERE id = $2",
        )
        .bind(auth.user_id)
        .bind(player_id)
        .execute(&state.pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE futsal_session_players SET is_paid = false, paid_at = NULL, marked_paid_by = NULL WHERE id = $1",
        )
        .bind(player_id)
        .execute(&state.pool)
        .await?;
    }

    get_session_inner(&state.pool, session_id).await
}

/// GET /api/accounting/users
pub async fn list_users(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<UserListItem>>> {
    let users = sqlx::query_as::<_, UserListItem>(
        "SELECT id, username, full_name FROM users ORDER BY full_name, username",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(users))
}

/// GET /api/accounting/user-summary
pub async fn user_summary(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<UserSummaryItem>>> {
    #[derive(sqlx::FromRow)]
    struct RawSummary {
        user_id: Option<Uuid>,
        player_name: String,
        total_due: Decimal,
        total_paid: Decimal,
        total_unpaid: Decimal,
        sessions_count: i64,
    }

    let rows = sqlx::query_as::<_, RawSummary>(
        r#"SELECT
             fsp.user_id,
             fsp.player_name,
             COALESCE(SUM(fsp.amount_due), 0) AS total_due,
             COALESCE(SUM(CASE WHEN fsp.is_paid THEN fsp.amount_due ELSE 0 END), 0) AS total_paid,
             COALESCE(SUM(CASE WHEN NOT fsp.is_paid THEN fsp.amount_due ELSE 0 END), 0) AS total_unpaid,
             COUNT(DISTINCT fsp.session_id) AS sessions_count
           FROM futsal_session_players fsp
           GROUP BY fsp.user_id, fsp.player_name
           ORDER BY total_unpaid DESC"#,
    )
    .fetch_all(&state.pool)
    .await?;

    let result: Vec<UserSummaryItem> = rows
        .into_iter()
        .map(|r| UserSummaryItem {
            user_id: r.user_id,
            player_name: r.player_name,
            total_due: r.total_due.to_string(),
            total_paid: r.total_paid.to_string(),
            total_unpaid: r.total_unpaid.to_string(),
            sessions_count: r.sessions_count,
        })
        .collect();

    Ok(Json(result))
}

// ── Authenticated user route ────────────────────────────────────────────────

/// GET /api/accounting/my-dues
pub async fn my_dues(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<Json<Vec<UserDue>>> {
    #[derive(sqlx::FromRow)]
    struct RawDue {
        session_id: Uuid,
        session_title: String,
        player_entry_id: Uuid,
        amount_due: Decimal,
        is_paid: bool,
    }

    let rows = sqlx::query_as::<_, RawDue>(
        r#"SELECT
             fs.id AS session_id,
             fs.title AS session_title,
             fsp.id AS player_entry_id,
             fsp.amount_due,
             fsp.is_paid
           FROM futsal_session_players fsp
           JOIN futsal_sessions fs ON fs.id = fsp.session_id
           WHERE fsp.user_id = $1
           ORDER BY fsp.is_paid ASC, fs.created_at DESC"#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await?;

    let result: Vec<UserDue> = rows
        .into_iter()
        .map(|r| UserDue {
            session_id: r.session_id,
            session_title: r.session_title,
            player_entry_id: r.player_entry_id,
            amount_due: r.amount_due.to_string(),
            is_paid: r.is_paid,
        })
        .collect();

    Ok(Json(result))
}

// ── Internal helper ─────────────────────────────────────────────────────────

async fn get_session_inner(
    pool: &sqlx::PgPool,
    session_id: Uuid,
) -> AppResult<Json<SessionDetailResponse>> {
    let row = sqlx::query_as::<_, FutsalSessionRow>(
        "SELECT id, title, total_amount, created_by, created_at FROM futsal_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".into()))?;

    let players = sqlx::query_as::<_, SessionPlayerRow>(
        r#"SELECT id, session_id, user_id, player_name, amount_due, is_paid, paid_at, marked_paid_by
           FROM futsal_session_players
           WHERE session_id = $1
           ORDER BY is_paid ASC, player_name ASC"#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    let player_count = players.len() as i64;
    let paid_count = players.iter().filter(|p| p.is_paid).count() as i64;

    Ok(Json(SessionDetailResponse {
        session: format_session(row, player_count, paid_count),
        players: players.into_iter().map(format_player).collect(),
    }))
}
