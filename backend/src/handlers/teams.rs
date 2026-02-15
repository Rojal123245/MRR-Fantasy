use axum::{
    extract::{Extension, Path, State},
    Json,
};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{CreateTeamRequest, FantasyTeam, FantasyTeamWithPlayers, Player, SetPlayersRequest};

/// POST /api/teams
///
/// Create a new fantasy team for the authenticated user.
pub async fn create_team(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateTeamRequest>,
) -> AppResult<Json<FantasyTeam>> {
    if body.name.is_empty() {
        return Err(AppError::BadRequest("Team name cannot be empty".to_string()));
    }

    // Check if user already has a team
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM fantasy_teams WHERE user_id = $1",
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    if existing > 0 {
        return Err(AppError::Conflict("You already have a fantasy team".to_string()));
    }

    let team = sqlx::query_as::<_, FantasyTeam>(
        r#"INSERT INTO fantasy_teams (user_id, name)
           VALUES ($1, $2)
           RETURNING id, user_id, name, created_at"#,
    )
    .bind(auth.user_id)
    .bind(&body.name)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(team))
}

/// GET /api/teams/my
///
/// Get the authenticated user's fantasy team with starters and bench.
pub async fn get_my_team(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<Json<FantasyTeamWithPlayers>> {
    let team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, created_at FROM fantasy_teams WHERE user_id = $1",
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("You don't have a fantasy team yet".to_string()))?;

    let starters = sqlx::query_as::<_, Player>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player, p.team_name, p.photo_url, p.price, p.total_points, p.created_at
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = false"#,
    )
    .bind(team.id)
    .fetch_all(&state.pool)
    .await?;

    let bench = sqlx::query_as::<_, Player>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player, p.team_name, p.photo_url, p.price, p.total_points, p.created_at
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = true"#,
    )
    .bind(team.id)
    .fetch_all(&state.pool)
    .await?;

    let total_points: i32 = starters.iter().map(|p| p.total_points).sum();

    Ok(Json(FantasyTeamWithPlayers {
        id: team.id,
        user_id: team.user_id,
        name: team.name,
        created_at: team.created_at,
        players: starters,
        bench,
        total_points,
    }))
}

/// PUT /api/teams/:id/players
///
/// Set the 9 players on a team (6 starters + 3 bench). Replaces existing selections.
/// Bench must contain exactly 1 GK and 2 outfield players (DEF/MID/FWD).
pub async fn set_team_players(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(team_id): Path<Uuid>,
    Json(body): Json<SetPlayersRequest>,
) -> AppResult<Json<FantasyTeamWithPlayers>> {
    // Validate exactly 6 starters
    if body.player_ids.len() != 6 {
        return Err(AppError::BadRequest("You must select exactly 6 starting players".to_string()));
    }

    // Validate exactly 3 bench players
    if body.bench_player_ids.len() != 3 {
        return Err(AppError::BadRequest("You must select exactly 3 bench players".to_string()));
    }

    // Combine all player IDs and check for duplicates
    let mut all_ids = body.player_ids.clone();
    all_ids.extend(&body.bench_player_ids);
    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    if unique.len() != 9 {
        return Err(AppError::BadRequest("Duplicate players are not allowed across starters and bench".to_string()));
    }

    // Verify team ownership
    let team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    // Verify all 9 players exist
    let valid_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM players WHERE id = ANY($1)",
    )
    .bind(&all_ids)
    .fetch_one(&state.pool)
    .await?;

    if valid_count != 9 {
        return Err(AppError::BadRequest("One or more player IDs are invalid".to_string()));
    }

    // Enforce max 2 top players across entire squad (starters + bench)
    let top_player_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM players WHERE id = ANY($1) AND is_top_player = true",
    )
    .bind(&all_ids)
    .fetch_one(&state.pool)
    .await?;

    if top_player_count > 2 {
        return Err(AppError::BadRequest(
            "Maximum 2 top players allowed per team (starters + bench combined)".to_string(),
        ));
    }

    // Enforce $70 budget cap across all 9 players
    let total_cost = sqlx::query_scalar::<_, rust_decimal::Decimal>(
        "SELECT COALESCE(SUM(price), 0) FROM players WHERE id = ANY($1)",
    )
    .bind(&all_ids)
    .fetch_one(&state.pool)
    .await?;

    let budget_limit = rust_decimal::Decimal::from(70);
    if total_cost > budget_limit {
        return Err(AppError::BadRequest(
            format!("Team cost ${total_cost} exceeds the $70 budget. Remove expensive players to fit the budget."),
        ));
    }

    // Validate bench composition: exactly 1 GK + 2 outfield (DEF/MID/FWD)
    let bench_gk_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM players WHERE id = ANY($1) AND position = 'GK'",
    )
    .bind(&body.bench_player_ids)
    .fetch_one(&state.pool)
    .await?;

    if bench_gk_count != 1 {
        return Err(AppError::BadRequest(
            "Bench must include exactly 1 goalkeeper (GK)".to_string(),
        ));
    }

    // Replace team players in a transaction
    let mut tx = state.pool.begin().await?;

    sqlx::query("DELETE FROM team_players WHERE team_id = $1")
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

    // Insert starters
    for player_id in &body.player_ids {
        sqlx::query("INSERT INTO team_players (team_id, player_id, is_bench) VALUES ($1, $2, false)")
            .bind(team_id)
            .bind(player_id)
            .execute(&mut *tx)
            .await?;
    }

    // Insert bench players
    for player_id in &body.bench_player_ids {
        sqlx::query("INSERT INTO team_players (team_id, player_id, is_bench) VALUES ($1, $2, true)")
            .bind(team_id)
            .bind(player_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    // Return updated team
    let starters = sqlx::query_as::<_, Player>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player, p.team_name, p.photo_url, p.price, p.total_points, p.created_at
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = false"#,
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;

    let bench = sqlx::query_as::<_, Player>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player, p.team_name, p.photo_url, p.price, p.total_points, p.created_at
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = true"#,
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;

    let total_points: i32 = starters.iter().map(|p| p.total_points).sum();

    Ok(Json(FantasyTeamWithPlayers {
        id: team.id,
        user_id: team.user_id,
        name: team.name,
        created_at: team.created_at,
        players: starters,
        bench,
        total_points,
    }))
}

/// GET /api/teams/:id/points
///
/// Get a team's total points breakdown (only starters count for points).
pub async fn get_team_points(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let starters = sqlx::query_as::<_, Player>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player, p.team_name, p.photo_url, p.price, p.total_points, p.created_at
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = false"#,
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;

    let bench = sqlx::query_as::<_, Player>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player, p.team_name, p.photo_url, p.price, p.total_points, p.created_at
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = true"#,
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;

    let total: i32 = starters.iter().map(|p| p.total_points).sum();

    Ok(Json(serde_json::json!({
        "team_id": team_id,
        "total_points": total,
        "players": starters,
        "bench": bench,
    })))
}
