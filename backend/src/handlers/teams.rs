use axum::{
    extract::{Extension, Path, State},
    Json,
};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateTeamRequest, FantasyTeam, FantasyTeamWithPlayers, Player, PlayerPosition,
    SetPlayersRequest, StarterPlayer,
};

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
           RETURNING id, user_id, name, captain_id, created_at"#,
    )
    .bind(auth.user_id)
    .bind(&body.name)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(team))
}

/// Row type for starter queries that includes the assigned position.
#[derive(Debug, sqlx::FromRow)]
struct StarterRow {
    // Player fields
    id: Uuid,
    name: String,
    position: PlayerPosition,
    secondary_position: Option<PlayerPosition>,
    is_top_player: bool,
    team_name: String,
    photo_url: Option<String>,
    price: rust_decimal::Decimal,
    total_points: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    // Assigned position from team_players
    assigned_position: Option<PlayerPosition>,
}

impl StarterRow {
    fn into_starter_player(self) -> StarterPlayer {
        let assigned = self
            .assigned_position
            .unwrap_or_else(|| self.position.clone());
        StarterPlayer {
            player: Player {
                id: self.id,
                name: self.name,
                position: self.position,
                secondary_position: self.secondary_position,
                is_top_player: self.is_top_player,
                team_name: self.team_name,
                photo_url: self.photo_url,
                price: self.price,
                total_points: self.total_points,
                created_at: self.created_at,
            },
            assigned_position: assigned,
        }
    }
}

/// Build a `FantasyTeamWithPlayers` from a team row by querying starters and bench.
async fn build_team_response(
    pool: &sqlx::PgPool,
    team: &FantasyTeam,
) -> Result<FantasyTeamWithPlayers, AppError> {
    let starter_rows = sqlx::query_as::<_, StarterRow>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player,
                  p.team_name, p.photo_url, p.price, p.total_points, p.created_at,
                  tp.assigned_position
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = false"#,
    )
    .bind(team.id)
    .fetch_all(pool)
    .await?;

    let starters: Vec<StarterPlayer> = starter_rows
        .into_iter()
        .map(|r| r.into_starter_player())
        .collect();

    let bench = sqlx::query_as::<_, Player>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player,
                  p.team_name, p.photo_url, p.price, p.total_points, p.created_at
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = true"#,
    )
    .bind(team.id)
    .fetch_all(pool)
    .await?;

    let total_points: i32 = starters.iter().map(|s| s.player.total_points).sum();

    Ok(FantasyTeamWithPlayers {
        id: team.id,
        user_id: team.user_id,
        name: team.name.clone(),
        captain_id: team.captain_id,
        created_at: team.created_at,
        players: starters,
        bench,
        total_points,
    })
}

/// GET /api/teams/my
///
/// Get the authenticated user's fantasy team with starters and bench.
pub async fn get_my_team(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<Json<FantasyTeamWithPlayers>> {
    let team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, created_at FROM fantasy_teams WHERE user_id = $1",
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("You don't have a fantasy team yet".to_string()))?;

    let response = build_team_response(&state.pool, &team).await?;
    Ok(Json(response))
}

/// PUT /api/teams/:id/players
///
/// Set the 9 players on a team (6 starters with assigned positions + 3 bench + captain).
/// Replaces existing selections.
///
/// Formation rules:
///   - Exactly 6 starters, each with an assigned_position
///   - Exactly 1 GK, at least 1 DEF, at least 1 MID, at least 1 FWD
///   - Each player's assigned_position must match their position or secondary_position
///   - Bench: exactly 1 GK + 2 outfield (DEF/MID/FWD)
///   - Captain must be one of the 6 starters
///   - Captain's name must NOT match the user's full_name (case-insensitive)
pub async fn set_team_players(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(team_id): Path<Uuid>,
    Json(body): Json<SetPlayersRequest>,
) -> AppResult<Json<FantasyTeamWithPlayers>> {
    // Validate exactly 6 starters
    if body.starters.len() != 6 {
        return Err(AppError::BadRequest(
            "You must select exactly 6 starting players".to_string(),
        ));
    }

    // Validate exactly 3 bench players
    if body.bench_player_ids.len() != 3 {
        return Err(AppError::BadRequest(
            "You must select exactly 3 bench players".to_string(),
        ));
    }

    // Captain must be one of the starters
    let starter_ids: Vec<Uuid> = body.starters.iter().map(|s| s.player_id).collect();
    if !starter_ids.contains(&body.captain_id) {
        return Err(AppError::BadRequest(
            "Captain must be one of the 6 starting players".to_string(),
        ));
    }

    // Combine all player IDs and check for duplicates
    let mut all_ids = starter_ids.clone();
    all_ids.extend(&body.bench_player_ids);
    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    if unique.len() != 9 {
        return Err(AppError::BadRequest(
            "Duplicate players are not allowed across starters and bench".to_string(),
        ));
    }

    // Validate formation: exactly 1 GK, at least 1 DEF, at least 1 MID, at least 1 FWD
    let mut gk_count = 0u8;
    let mut def_count = 0u8;
    let mut mid_count = 0u8;
    let mut fwd_count = 0u8;
    for assignment in &body.starters {
        match assignment.assigned_position {
            PlayerPosition::Gk => gk_count += 1,
            PlayerPosition::Def => def_count += 1,
            PlayerPosition::Mid => mid_count += 1,
            PlayerPosition::Fwd => fwd_count += 1,
        }
    }

    if gk_count != 1 {
        return Err(AppError::BadRequest(
            "Starting lineup must have exactly 1 GK".to_string(),
        ));
    }
    if def_count < 1 {
        return Err(AppError::BadRequest(
            "Starting lineup must have at least 1 DEF".to_string(),
        ));
    }
    if mid_count < 1 {
        return Err(AppError::BadRequest(
            "Starting lineup must have at least 1 MID".to_string(),
        ));
    }
    if fwd_count < 1 {
        return Err(AppError::BadRequest(
            "Starting lineup must have at least 1 FWD".to_string(),
        ));
    }

    // Verify team ownership
    let team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    // Verify all 9 players exist and fetch starter details for position validation
    let valid_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM players WHERE id = ANY($1)",
    )
    .bind(&all_ids)
    .fetch_one(&state.pool)
    .await?;

    if valid_count != 9 {
        return Err(AppError::BadRequest(
            "One or more player IDs are invalid".to_string(),
        ));
    }

    // Validate that each starter's assigned_position matches their position or secondary_position
    let starter_players = sqlx::query_as::<_, Player>(
        r#"SELECT id, name, position, secondary_position, is_top_player,
                  team_name, photo_url, price, total_points, created_at
           FROM players WHERE id = ANY($1)"#,
    )
    .bind(&starter_ids)
    .fetch_all(&state.pool)
    .await?;

    for assignment in &body.starters {
        let player = starter_players
            .iter()
            .find(|p| p.id == assignment.player_id)
            .ok_or_else(|| {
                AppError::BadRequest(format!("Player {} not found", assignment.player_id))
            })?;

        let matches_primary = player.position == assignment.assigned_position;
        let matches_secondary = player
            .secondary_position
            .as_ref()
            .map_or(false, |sp| *sp == assignment.assigned_position);

        if !matches_primary && !matches_secondary {
            return Err(AppError::BadRequest(format!(
                "{} cannot play as {:?}. Valid positions: {:?}{}",
                player.name,
                assignment.assigned_position,
                player.position,
                player
                    .secondary_position
                    .as_ref()
                    .map_or(String::new(), |sp| format!(", {:?}", sp))
            )));
        }
    }

    // Validate captain: name must NOT match user's full_name (case-insensitive)
    let user_full_name = sqlx::query_scalar::<_, String>(
        "SELECT full_name FROM users WHERE id = $1",
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    let captain_player = starter_players
        .iter()
        .find(|p| p.id == body.captain_id)
        .ok_or_else(|| AppError::BadRequest("Captain player not found".to_string()))?;

    if captain_player.name.trim().eq_ignore_ascii_case(user_full_name.trim()) {
        return Err(AppError::BadRequest(format!(
            "You cannot captain {} because they share your name. Choose a different captain.",
            captain_player.name
        )));
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
        return Err(AppError::BadRequest(format!(
            "Team cost ${total_cost} exceeds the $70 budget. Remove expensive players to fit the budget."
        )));
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

    // Insert starters with assigned positions
    for assignment in &body.starters {
        sqlx::query(
            "INSERT INTO team_players (team_id, player_id, is_bench, assigned_position) VALUES ($1, $2, false, $3)",
        )
        .bind(team_id)
        .bind(assignment.player_id)
        .bind(&assignment.assigned_position)
        .execute(&mut *tx)
        .await?;
    }

    // Insert bench players (no assigned_position)
    for player_id in &body.bench_player_ids {
        sqlx::query(
            "INSERT INTO team_players (team_id, player_id, is_bench) VALUES ($1, $2, true)",
        )
        .bind(team_id)
        .bind(player_id)
        .execute(&mut *tx)
        .await?;
    }

    // Update captain
    sqlx::query("UPDATE fantasy_teams SET captain_id = $1 WHERE id = $2")
        .bind(body.captain_id)
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // Return updated team (re-fetch to get updated captain_id)
    let updated_team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, created_at FROM fantasy_teams WHERE id = $1",
    )
    .bind(team_id)
    .fetch_one(&state.pool)
    .await?;

    let response = build_team_response(&state.pool, &updated_team).await?;
    Ok(Json(response))
}

/// GET /api/teams/:id/points
///
/// Get a team's total points breakdown (only starters count for points).
pub async fn get_team_points(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, created_at FROM fantasy_teams WHERE id = $1",
    )
    .bind(team_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found".to_string()))?;

    let starter_rows = sqlx::query_as::<_, StarterRow>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player,
                  p.team_name, p.photo_url, p.price, p.total_points, p.created_at,
                  tp.assigned_position
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = false"#,
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;

    let starters: Vec<StarterPlayer> = starter_rows
        .into_iter()
        .map(|r| r.into_starter_player())
        .collect();

    let bench = sqlx::query_as::<_, Player>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player,
                  p.team_name, p.photo_url, p.price, p.total_points, p.created_at
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = true"#,
    )
    .bind(team_id)
    .fetch_all(&state.pool)
    .await?;

    let total: i32 = starters.iter().map(|s| s.player.total_points).sum();

    Ok(Json(serde_json::json!({
        "team_id": team_id,
        "captain_id": team.captain_id,
        "total_points": total,
        "players": starters,
        "bench": bench,
    })))
}
