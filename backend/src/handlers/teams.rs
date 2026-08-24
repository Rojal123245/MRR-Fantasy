use axum::{
    extract::{Extension, Path, State},
    Json,
};
use chrono::{Datelike, Timelike, Utc};
use chrono_tz::America::New_York;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateTeamRequest, FantasyTeam, FantasyTeamWithPlayers, Player, PlayerPosition,
    SetPlayersRequest, StarterPlayer, TransferRecord, TransferRequest, TransferStatusResponse,
};
use crate::services::points_sql;
use crate::services::scoring;

#[derive(Debug, Serialize)]
pub struct LockStatusResponse {
    pub locked: bool,
    pub unlock_at: Option<String>,
    pub manually_unlocked: bool,
    pub active_gameweek: Option<i32>,
}

/// The lock window, evaluated against a given Eastern-time instant.
///
/// The deadline is the end of Saturday: teams are locked for the whole of Sunday
/// morning and reopen at noon. Saturday itself needs no clause — the rule is now
/// entirely "is it Sunday morning".
///
/// Deliberately expressed in wall-clock Eastern time, so the window is midnight
/// to noon as managers read a clock. On the two Sundays a year the clocks move
/// that makes the window 11 or 13 real hours; the alternative would shift the
/// deadline by an hour twice a season, which is worse.
fn scheduled_lock_status_at(now_et: chrono::DateTime<chrono_tz::Tz>) -> (bool, Option<String>) {
    let locked = matches!(now_et.weekday(), chrono::Weekday::Sun) && now_et.hour() < 12;

    // Only Sunday morning is ever locked, so the unlock is always noon today.
    let unlock_at = if locked {
        now_et
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .and_then(|dt| dt.and_local_timezone(New_York).single())
            .map(|t| t.to_rfc3339())
    } else {
        None
    };

    (locked, unlock_at)
}

fn scheduled_lock_status() -> (bool, Option<String>) {
    scheduled_lock_status_at(Utc::now().with_timezone(&New_York))
}

/// The schedule with the admin override applied.
///
/// `force_unlock` in `lineup_lock_control` always wins, so an admin can reopen
/// team selection mid-window without waiting for noon.
fn effective_lock(
    scheduled: (bool, Option<String>),
    manually_unlocked: bool,
) -> (bool, Option<String>) {
    let (scheduled_locked, unlock_at) = scheduled;
    let locked = scheduled_locked && !manually_unlocked;
    (locked, if locked { unlock_at } else { None })
}

pub async fn compute_lock_status(pool: &sqlx::PgPool) -> AppResult<LockStatusResponse> {
    let manually_unlocked = sqlx::query_scalar::<_, bool>(
        "SELECT force_unlock FROM lineup_lock_control WHERE id = true",
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(false);

    let active_gameweek = sqlx::query_scalar::<_, i32>(
        "SELECT week_number FROM match_weeks WHERE is_active = true LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    let (locked, unlock_at) = effective_lock(scheduled_lock_status(), manually_unlocked);

    Ok(LockStatusResponse {
        locked,
        unlock_at,
        manually_unlocked,
        active_gameweek,
    })
}

/// GET /api/teams/lock-status
///
/// Returns whether lineup changes are currently locked.
pub async fn lock_status(State(state): State<AppState>) -> AppResult<Json<LockStatusResponse>> {
    Ok(Json(compute_lock_status(&state.pool).await?))
}

/// POST /api/teams
///
/// Create a new fantasy team for the authenticated user.
pub async fn create_team(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateTeamRequest>,
) -> AppResult<Json<FantasyTeam>> {
    if body.name.is_empty() {
        return Err(AppError::BadRequest(
            "Team name cannot be empty".to_string(),
        ));
    }

    // Check if user already has a team
    let existing =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM fantasy_teams WHERE user_id = $1")
            .bind(auth.user_id)
            .fetch_one(&state.pool)
            .await?;

    if existing > 0 {
        return Err(AppError::Conflict(
            "You already have a fantasy team".to_string(),
        ));
    }

    let team = sqlx::query_as::<_, FantasyTeam>(
        r#"INSERT INTO fantasy_teams (user_id, name)
           VALUES ($1, $2)
           RETURNING id, user_id, name, captain_id, budget_limit, created_at"#,
    )
    .bind(auth.user_id)
    .bind(&body.name)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(team))
}

/// Fetch a team's 6 starters with the points they earned for that team.
pub async fn fetch_team_starters(
    pool: &sqlx::PgPool,
    team_id: Uuid,
) -> Result<Vec<StarterPlayer>, AppError> {
    let rows = sqlx::query_as::<_, StarterRow>(&points_sql::squad_season_points(false))
        .bind(team_id)
        .fetch_all(pool)
        .await?;

    Ok(rows.into_iter().map(|r| r.into_starter_player()).collect())
}

/// Fetch a team's 3 bench players. Points reflect the role their manager assigned,
/// and only ever counted towards a total in a Bench Boost week.
pub async fn fetch_team_bench(
    pool: &sqlx::PgPool,
    team_id: Uuid,
) -> Result<Vec<Player>, AppError> {
    let rows = sqlx::query_as::<_, StarterRow>(&points_sql::squad_season_points(true))
        .bind(team_id)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| r.into_starter_player().player)
        .collect())
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
    let starters = fetch_team_starters(pool, team.id).await?;
    let bench = fetch_team_bench(pool, team.id).await?;

    let total_points = team_total_points(pool, team.id).await?;

    Ok(FantasyTeamWithPlayers {
        id: team.id,
        user_id: team.user_id,
        name: team.name.clone(),
        captain_id: team.captain_id,
        budget_limit: team.budget_limit,
        created_at: team.created_at,
        players: starters,
        bench,
        total_points,
    })
}

async fn team_total_points(pool: &sqlx::PgPool, team_id: Uuid) -> Result<i32, AppError> {
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(total_points), 0) FROM team_gameweek_points WHERE team_id = $1",
    )
    .bind(team_id)
    .fetch_one(pool)
    .await?;

    Ok(total as i32)
}

async fn snapshot_team_lineup_if_missing(
    pool: &sqlx::PgPool,
    team_id: Uuid,
    match_week_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    scoring::snapshot_team_lineup(&mut tx, team_id, match_week_id).await?;
    tx.commit().await?;
    Ok(())
}

/// GET /api/teams/my
///
/// Get the authenticated user's fantasy team with starters and bench.
pub async fn get_my_team(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<Json<FantasyTeamWithPlayers>> {
    let team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE user_id = $1",
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
    let lock = compute_lock_status(&state.pool).await?;
    if lock.locked {
        return Err(AppError::BadRequest(
            "Team selection closes at the end of Saturday. Lineups are locked until Sunday 12:00 PM ET.".to_string(),
        ));
    }

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
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    // When a gameweek is active, only allow rearranging existing squad (no new players).
    // To bring in new players, use the transfer endpoint.
    let active_week_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM match_weeks WHERE is_active = true LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some(week_id) = active_week_id {
        let mut conn = state.pool.acquire().await?;
        if scoring::week_already_scored(&mut conn, week_id).await? {
            return Err(AppError::BadRequest(
                "That gameweek has already been scored and is closed. Lineup changes \
                 will apply from the next gameweek."
                    .to_string(),
            ));
        }
        drop(conn);
        snapshot_team_lineup_if_missing(&state.pool, team_id, week_id).await?;
    }

    let current_player_ids =
        sqlx::query_scalar::<_, Uuid>("SELECT player_id FROM team_players WHERE team_id = $1")
            .bind(team_id)
            .fetch_all(&state.pool)
            .await?;

    if active_week_id.is_some() && !current_player_ids.is_empty() {
        let current_set: std::collections::HashSet<Uuid> =
            current_player_ids.iter().cloned().collect();
        let new_ids: Vec<Uuid> = all_ids
            .iter()
            .filter(|id| !current_set.contains(id))
            .cloned()
            .collect();
        if !new_ids.is_empty() {
            return Err(AppError::BadRequest(
                "A gameweek is active — you can only rearrange your existing 9 players. Use the Transfer feature to make swaps."
                    .to_string(),
            ));
        }
    }

    // Verify all 9 players exist and fetch starter details for position validation
    let valid_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM players WHERE id = ANY($1)")
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
    let user_full_name =
        sqlx::query_scalar::<_, String>("SELECT full_name FROM users WHERE id = $1")
            .bind(auth.user_id)
            .fetch_one(&state.pool)
            .await?;

    let captain_player = starter_players
        .iter()
        .find(|p| p.id == body.captain_id)
        .ok_or_else(|| AppError::BadRequest("Captain player not found".to_string()))?;

    if captain_player
        .name
        .trim()
        .eq_ignore_ascii_case(user_full_name.trim())
    {
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

    if total_cost > team.budget_limit {
        return Err(AppError::BadRequest(format!(
            "Team cost ${total_cost} exceeds your ${} budget. Remove expensive players to fit the budget.",
            team.budget_limit
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
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1",
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
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1",
    )
    .bind(team_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found".to_string()))?;

    let starters = fetch_team_starters(&state.pool, team_id).await?;
    let bench = fetch_team_bench(&state.pool, team_id).await?;

    let total = team_total_points(&state.pool, team_id).await?;

    Ok(Json(serde_json::json!({
        "team_id": team_id,
        "captain_id": team.captain_id,
        "total_points": total,
        "players": starters,
        "bench": bench,
    })))
}

/// GET /api/teams/:id/transfer-status
///
/// Check transfer usage and points hit for the current gameweek.
pub async fn get_transfer_status(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(team_id): Path<Uuid>,
) -> AppResult<Json<TransferStatusResponse>> {
    let _team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    #[derive(sqlx::FromRow)]
    struct ActiveWeekRow {
        id: Uuid,
        week_number: i32,
    }

    let active_week = sqlx::query_as::<_, ActiveWeekRow>(
        "SELECT id, week_number FROM match_weeks WHERE is_active = true LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;

    let Some(week) = active_week else {
        return Ok(Json(TransferStatusResponse {
            transfer_available: false,
            active_gameweek: None,
            transfers_used: 0,
            free_transfers: 1,
            extra_transfers: 0,
            points_hit: 0,
            transferred_out: None,
            transferred_in: None,
        }));
    };

    let transfers_used = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM transfers WHERE team_id = $1 AND match_week_id = $2",
    )
    .bind(team_id)
    .bind(week.id)
    .fetch_one(&state.pool)
    .await?;

    let latest_transfer = sqlx::query_as::<_, TransferRecord>(
        "SELECT id, team_id, match_week_id, player_out_id, player_in_id, created_at
         FROM transfers
         WHERE team_id = $1 AND match_week_id = $2
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(team_id)
    .bind(week.id)
    .fetch_optional(&state.pool)
    .await?;

    let (transferred_out, transferred_in) = if let Some(transfer) = latest_transfer {
        let out_name = sqlx::query_scalar::<_, String>("SELECT name FROM players WHERE id = $1")
            .bind(transfer.player_out_id)
            .fetch_optional(&state.pool)
            .await?;
        let in_name = sqlx::query_scalar::<_, String>("SELECT name FROM players WHERE id = $1")
            .bind(transfer.player_in_id)
            .fetch_optional(&state.pool)
            .await?;
        (out_name, in_name)
    } else {
        (None, None)
    };

    let transfers_used_i32 = transfers_used as i32;
    let free_transfers = 1;
    let extra_transfers = (transfers_used_i32 - free_transfers).max(0);
    let points_hit = extra_transfers * 4;

    Ok(Json(TransferStatusResponse {
        transfer_available: true,
        active_gameweek: Some(week.week_number),
        transfers_used: transfers_used_i32,
        free_transfers,
        extra_transfers,
        points_hit,
        transferred_out,
        transferred_in,
    }))
}

/// POST /api/teams/:id/transfer
///
/// Transfer 1 player: swap player_out (must be in squad) for player_in (new player).
/// First transfer each active gameweek is free; each additional transfer costs -4 points.
pub async fn transfer_player(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(team_id): Path<Uuid>,
    Json(body): Json<TransferRequest>,
) -> AppResult<Json<FantasyTeamWithPlayers>> {
    let lock = compute_lock_status(&state.pool).await?;
    if lock.locked {
        return Err(AppError::BadRequest(
            "Transfers close at the end of Saturday. They reopen Sunday 12:00 PM ET.".to_string(),
        ));
    }

    let team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    #[derive(sqlx::FromRow)]
    struct ActiveWeekRow {
        id: Uuid,
        #[allow(dead_code)]
        week_number: i32,
    }

    let active_week = sqlx::query_as::<_, ActiveWeekRow>(
        "SELECT id, week_number FROM match_weeks WHERE is_active = true LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(
            "No active gameweek. Transfers are only available during a gameweek.".to_string(),
        )
    })?;

    let mut conn = state.pool.acquire().await?;
    if scoring::week_already_scored(&mut conn, active_week.id).await? {
        return Err(AppError::BadRequest(
            "That gameweek has already been scored and is closed. Transfers will be \
             available again when the next gameweek opens."
                .to_string(),
        ));
    }
    drop(conn);

    snapshot_team_lineup_if_missing(&state.pool, team_id, active_week.id).await?;

    if body.player_out_id == body.player_in_id {
        return Err(AppError::BadRequest(
            "Player out and player in cannot be the same".to_string(),
        ));
    }

    #[derive(sqlx::FromRow)]
    struct TeamPlayerSlot {
        is_bench: bool,
        assigned_position: Option<PlayerPosition>,
    }

    let outgoing_slot = sqlx::query_as::<_, TeamPlayerSlot>(
        "SELECT is_bench, assigned_position FROM team_players WHERE team_id = $1 AND player_id = $2",
    )
    .bind(team_id)
    .bind(body.player_out_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest("The player you want to transfer out is not in your squad".to_string())
    })?;

    let in_already = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_players WHERE team_id = $1 AND player_id = $2",
    )
    .bind(team_id)
    .bind(body.player_in_id)
    .fetch_one(&state.pool)
    .await?;

    if in_already > 0 {
        return Err(AppError::BadRequest(
            "The player you want to transfer in is already in your squad".to_string(),
        ));
    }

    let incoming = sqlx::query_as::<_, Player>(
        r#"SELECT id, name, position, secondary_position, is_top_player,
                  team_name, photo_url, price, total_points, created_at
           FROM players WHERE id = $1"#,
    )
    .bind(body.player_in_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Player to transfer in not found".to_string()))?;

    let outgoing = sqlx::query_as::<_, Player>(
        r#"SELECT id, name, position, secondary_position, is_top_player,
                  team_name, photo_url, price, total_points, created_at
           FROM players WHERE id = $1"#,
    )
    .bind(body.player_out_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Player to transfer out not found".to_string()))?;

    let final_position = if outgoing_slot.is_bench {
        None
    } else {
        let pos = body.assigned_position.unwrap_or_else(|| {
            outgoing_slot
                .assigned_position
                .unwrap_or_else(|| incoming.position.clone())
        });
        Some(pos)
    };

    if let Some(ref pos) = final_position {
        let matches_primary = incoming.position == *pos;
        let matches_secondary = incoming
            .secondary_position
            .as_ref()
            .map_or(false, |sp| *sp == *pos);
        if !matches_primary && !matches_secondary {
            return Err(AppError::BadRequest(format!(
                "{} cannot play as {:?}. Valid positions: {:?}{}",
                incoming.name,
                pos,
                incoming.position,
                incoming
                    .secondary_position
                    .as_ref()
                    .map_or(String::new(), |sp| format!(", {:?}", sp))
            )));
        }
    }

    let other_player_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT player_id FROM team_players WHERE team_id = $1 AND player_id != $2",
    )
    .bind(team_id)
    .bind(body.player_out_id)
    .fetch_all(&state.pool)
    .await?;

    let mut all_ids: Vec<Uuid> = other_player_ids;
    all_ids.push(body.player_in_id);

    let top_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM players WHERE id = ANY($1) AND is_top_player = true",
    )
    .bind(&all_ids)
    .fetch_one(&state.pool)
    .await?;

    if top_count > 2 {
        return Err(AppError::BadRequest(
            "Transfer would exceed the 2 top-player limit".to_string(),
        ));
    }

    let total_cost = sqlx::query_scalar::<_, rust_decimal::Decimal>(
        "SELECT COALESCE(SUM(price), 0) FROM players WHERE id = ANY($1)",
    )
    .bind(&all_ids)
    .fetch_one(&state.pool)
    .await?;

    if total_cost > team.budget_limit {
        return Err(AppError::BadRequest(format!(
            "Transfer would push team cost to ${total_cost}, exceeding your ${} budget",
            team.budget_limit
        )));
    }

    if outgoing_slot.is_bench {
        let bench_gks_without_out = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM team_players tp
               JOIN players p ON p.id = tp.player_id
               WHERE tp.team_id = $1 AND tp.is_bench = true
               AND tp.player_id != $2 AND p.position = 'GK'"#,
        )
        .bind(team_id)
        .bind(body.player_out_id)
        .fetch_one(&state.pool)
        .await?;

        let incoming_is_gk = incoming.position == PlayerPosition::Gk;
        let outgoing_is_gk = outgoing.position == PlayerPosition::Gk;

        if outgoing_is_gk && !incoming_is_gk && bench_gks_without_out == 0 {
            return Err(AppError::BadRequest(
                "Bench must keep exactly 1 GK. Transfer in a GK to replace the bench GK."
                    .to_string(),
            ));
        }
        if !outgoing_is_gk && incoming_is_gk && bench_gks_without_out >= 1 {
            return Err(AppError::BadRequest(
                "Bench already has 1 GK. Cannot add another GK to the bench.".to_string(),
            ));
        }
    }

    if team.captain_id == Some(body.player_out_id) {
        return Err(AppError::BadRequest(
            "Cannot transfer out your captain. Change your captain first.".to_string(),
        ));
    }

    let mut tx = state.pool.begin().await?;

    sqlx::query("DELETE FROM team_players WHERE team_id = $1 AND player_id = $2")
        .bind(team_id)
        .bind(body.player_out_id)
        .execute(&mut *tx)
        .await?;

    if outgoing_slot.is_bench {
        sqlx::query(
            "INSERT INTO team_players (team_id, player_id, is_bench) VALUES ($1, $2, true)",
        )
        .bind(team_id)
        .bind(body.player_in_id)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO team_players (team_id, player_id, is_bench, assigned_position) VALUES ($1, $2, false, $3)",
        )
        .bind(team_id)
        .bind(body.player_in_id)
        .bind(&final_position)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO transfers (team_id, match_week_id, player_out_id, player_in_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(team_id)
    .bind(active_week.id)
    .bind(body.player_out_id)
    .bind(body.player_in_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let updated_team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1",
    )
    .bind(team_id)
    .fetch_one(&state.pool)
    .await?;

    let response = build_team_response(&state.pool, &updated_team).await?;
    Ok(Json(response))
}

#[cfg(test)]
mod lock_schedule_tests {
    use super::*;
    use chrono::offset::Offset;
    use chrono::NaiveDate;

    /// A wall-clock Eastern instant. `earliest` resolves the hour that repeats
    /// when the clocks go back; times that do not exist are never constructed.
    fn et(y: i32, m: u32, d: u32, h: u32, min: u32) -> chrono::DateTime<chrono_tz::Tz> {
        NaiveDate::from_ymd_opt(y, m, d)
            .expect("valid date")
            .and_hms_opt(h, min, 0)
            .expect("valid time")
            .and_local_timezone(New_York)
            .earliest()
            .expect("a real Eastern instant")
    }

    fn locked_at(t: chrono::DateTime<chrono_tz::Tz>) -> bool {
        scheduled_lock_status_at(t).0
    }

    /// The deadline is the end of Saturday. These four instants are the rule.
    ///
    /// 2026-08-22 is a Saturday, 2026-08-23 the Sunday after it.
    #[test]
    fn the_deadline_is_the_end_of_saturday() {
        assert!(
            !locked_at(et(2026, 8, 22, 23, 59)),
            "Saturday 23:59 ET is still open — the deadline has not passed"
        );
        assert!(
            locked_at(et(2026, 8, 23, 0, 0)),
            "Sunday 00:00 ET is locked — the deadline has just passed"
        );
        assert!(
            locked_at(et(2026, 8, 23, 11, 59)),
            "Sunday 11:59 ET is still locked"
        );
        assert!(
            !locked_at(et(2026, 8, 23, 12, 0)),
            "Sunday 12:00 ET reopens"
        );
    }

    /// Saturday used to lock from 22:00. Nothing on Saturday locks any more.
    #[test]
    fn saturday_evening_is_no_longer_locked() {
        for hour in [21, 22, 23] {
            assert!(
                !locked_at(et(2026, 8, 22, hour, 0)),
                "Saturday {hour}:00 ET must be open under the new deadline"
            );
        }
    }

    /// Every other day is open all day.
    #[test]
    fn the_rest_of_the_week_is_open() {
        // Monday 2026-08-24 through Friday 2026-08-28.
        for day in 24..=28 {
            for hour in [0, 6, 12, 23] {
                assert!(
                    !locked_at(et(2026, 8, day, hour, 0)),
                    "2026-08-{day} {hour}:00 ET must be open"
                );
            }
        }
    }

    /// The window is midnight to noon on the clock, including the Sunday the
    /// clocks jump forward — when 02:00 to 02:59 does not exist at all.
    ///
    /// 2026-03-08 is the second Sunday in March: EST becomes EDT at 02:00.
    #[test]
    fn spring_forward_sunday_still_locks_midnight_to_noon() {
        assert!(locked_at(et(2026, 3, 8, 0, 0)), "midnight, still EST");
        assert!(locked_at(et(2026, 3, 8, 1, 59)), "last minute before the jump");
        assert!(locked_at(et(2026, 3, 8, 3, 0)), "first hour after the jump, now EDT");
        assert!(locked_at(et(2026, 3, 8, 11, 59)));
        assert!(!locked_at(et(2026, 3, 8, 12, 0)), "noon EDT reopens");

        // The offset either side of the jump really did change, so this is a
        // genuine DST case and not a fixed -05:00 masquerading as one.
        assert_eq!(et(2026, 3, 8, 1, 59).offset().fix().to_string(), "-05:00");
        assert_eq!(et(2026, 3, 8, 3, 0).offset().fix().to_string(), "-04:00");

        // And the unlock instant is noon EDT, not noon EST.
        let (_, unlock_at) = scheduled_lock_status_at(et(2026, 3, 8, 3, 0));
        assert_eq!(unlock_at.as_deref(), Some("2026-03-08T12:00:00-04:00"));
    }

    /// And the Sunday the clocks go back, when 01:00 to 01:59 happens twice.
    ///
    /// 2026-11-01 is the first Sunday in November: EDT becomes EST at 02:00.
    #[test]
    fn fall_back_sunday_still_locks_midnight_to_noon() {
        let repeated = NaiveDate::from_ymd_opt(2026, 11, 1)
            .unwrap()
            .and_hms_opt(1, 30, 0)
            .unwrap();
        let first_pass = repeated.and_local_timezone(New_York).earliest().unwrap();
        let second_pass = repeated.and_local_timezone(New_York).latest().unwrap();
        assert_ne!(first_pass, second_pass, "01:30 really does happen twice");

        assert!(locked_at(first_pass), "01:30 EDT is locked");
        assert!(locked_at(second_pass), "01:30 EST, an hour later, still locked");
        assert!(locked_at(et(2026, 11, 1, 11, 59)));
        assert!(!locked_at(et(2026, 11, 1, 12, 0)), "noon EST reopens");

        let (_, unlock_at) = scheduled_lock_status_at(second_pass);
        assert_eq!(unlock_at.as_deref(), Some("2026-11-01T12:00:00-05:00"));
    }

    /// The timezone must be the DST-aware zone, not a fixed -05:00. If it were
    /// fixed, the deadline would drift by an hour for eight months of the year.
    #[test]
    fn eastern_time_is_dst_aware() {
        assert_eq!(et(2026, 1, 15, 12, 0).offset().fix().to_string(), "-05:00", "winter is EST");
        assert_eq!(et(2026, 7, 15, 12, 0).offset().fix().to_string(), "-04:00", "summer is EDT");
    }

    /// An admin can reopen team selection mid-window, and that still wins.
    #[test]
    fn force_unlock_overrides_the_schedule() {
        let sunday_morning = scheduled_lock_status_at(et(2026, 8, 23, 9, 0));
        assert!(sunday_morning.0, "the schedule says locked");

        let (locked, unlock_at) = effective_lock(sunday_morning.clone(), true);
        assert!(!locked, "force_unlock must reopen it");
        assert_eq!(unlock_at, None, "and there is nothing to count down to");

        let (locked, unlock_at) = effective_lock(sunday_morning, false);
        assert!(locked, "without the override the schedule stands");
        assert!(unlock_at.is_some(), "and it says when it reopens");
    }
}
