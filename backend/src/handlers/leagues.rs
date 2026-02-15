use axum::{
    extract::{Extension, Path, State},
    Json,
};
use rand::Rng;
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateLeagueRequest, JoinLeagueRequest, League, LeagueDetail, LeagueMemberStanding,
};

/// Generate a random 8-character alphanumeric invite code.
fn generate_invite_code() -> String {
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();
    (0..8).map(|_| chars[rng.gen_range(0..chars.len())]).collect()
}

/// POST /api/leagues
///
/// Create a new league and automatically add the creator as a member.
pub async fn create_league(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateLeagueRequest>,
) -> AppResult<Json<League>> {
    if body.name.is_empty() {
        return Err(AppError::BadRequest("League name cannot be empty".to_string()));
    }

    let invite_code = generate_invite_code();

    let mut tx = state.pool.begin().await?;

    let league = sqlx::query_as::<_, League>(
        r#"INSERT INTO leagues (name, invite_code, created_by)
           VALUES ($1, $2, $3)
           RETURNING id, name, invite_code, created_by, created_at"#,
    )
    .bind(&body.name)
    .bind(&invite_code)
    .bind(auth.user_id)
    .fetch_one(&mut *tx)
    .await?;

    // Auto-add creator as a member
    sqlx::query("INSERT INTO league_members (league_id, user_id) VALUES ($1, $2)")
        .bind(league.id)
        .bind(auth.user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(league))
}

/// POST /api/leagues/join
///
/// Join a league using an invite code.
pub async fn join_league(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<JoinLeagueRequest>,
) -> AppResult<Json<League>> {
    let league = sqlx::query_as::<_, League>(
        "SELECT id, name, invite_code, created_by, created_at FROM leagues WHERE invite_code = $1",
    )
    .bind(&body.invite_code)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Invalid invite code".to_string()))?;

    // Check if already a member
    let already_member = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM league_members WHERE league_id = $1 AND user_id = $2",
    )
    .bind(league.id)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    if already_member > 0 {
        return Err(AppError::Conflict("You are already a member of this league".to_string()));
    }

    sqlx::query("INSERT INTO league_members (league_id, user_id) VALUES ($1, $2)")
        .bind(league.id)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await?;

    Ok(Json(league))
}

/// GET /api/leagues/:id
///
/// Get league details including member standings.
pub async fn get_league(
    State(state): State<AppState>,
    Path(league_id): Path<Uuid>,
) -> AppResult<Json<LeagueDetail>> {
    let league = sqlx::query_as::<_, League>(
        "SELECT id, name, invite_code, created_by, created_at FROM leagues WHERE id = $1",
    )
    .bind(league_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("League not found".to_string()))?;

    let members = sqlx::query_as::<_, LeagueMemberStanding>(
        r#"SELECT
             u.id AS user_id,
             u.username,
             ft.name AS team_name,
             COALESCE(SUM(p.total_points), 0) AS total_points
           FROM league_members lm
           INNER JOIN users u ON u.id = lm.user_id
           LEFT JOIN fantasy_teams ft ON ft.user_id = u.id
           LEFT JOIN team_players tp ON tp.team_id = ft.id
           LEFT JOIN players p ON p.id = tp.player_id
           WHERE lm.league_id = $1
           GROUP BY u.id, u.username, ft.name
           ORDER BY total_points DESC"#,
    )
    .bind(league_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(LeagueDetail { league, members }))
}

/// GET /api/leagues/:id/leaderboard
///
/// Get ranked leaderboard for a league.
pub async fn get_leaderboard(
    State(state): State<AppState>,
    Path(league_id): Path<Uuid>,
) -> AppResult<Json<Vec<LeagueMemberStanding>>> {
    let standings = sqlx::query_as::<_, LeagueMemberStanding>(
        r#"SELECT
             u.id AS user_id,
             u.username,
             ft.name AS team_name,
             COALESCE(SUM(p.total_points), 0) AS total_points
           FROM league_members lm
           INNER JOIN users u ON u.id = lm.user_id
           LEFT JOIN fantasy_teams ft ON ft.user_id = u.id
           LEFT JOIN team_players tp ON tp.team_id = ft.id
           LEFT JOIN players p ON p.id = tp.player_id
           WHERE lm.league_id = $1
           GROUP BY u.id, u.username, ft.name
           ORDER BY total_points DESC"#,
    )
    .bind(league_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(standings))
}
