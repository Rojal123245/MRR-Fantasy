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

#[derive(Debug, Serialize)]
pub struct LockStatusResponse {
    pub locked: bool,
    pub unlock_at: Option<String>,
    pub manually_unlocked: bool,
}

fn scheduled_lock_status() -> (bool, Option<String>) {
    let now_et = Utc::now().with_timezone(&New_York);
    let weekday = now_et.weekday();

    let locked = matches!(weekday, chrono::Weekday::Sat)
        || (matches!(weekday, chrono::Weekday::Sun) && now_et.hour() < 12);

    let unlock_at = if locked {
        let days_until_sunday = if matches!(weekday, chrono::Weekday::Sat) {
            1
        } else {
            0
        };
        let unlock = (now_et + chrono::Duration::days(days_until_sunday))
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .map(|dt| {
                dt.and_local_timezone(New_York)
                    .single()
                    .map(|t| t.to_rfc3339())
            })
            .flatten();
        unlock
    } else {
        None
    };

    (locked, unlock_at)
}

pub async fn compute_lock_status(pool: &sqlx::PgPool) -> AppResult<LockStatusResponse> {
    let manually_unlocked = sqlx::query_scalar::<_, bool>(
        "SELECT force_unlock FROM lineup_lock_control WHERE id = true",
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(false);

    let (scheduled_locked, scheduled_unlock_at) = scheduled_lock_status();
    let locked = scheduled_locked && !manually_unlocked;
    let unlock_at = if locked { scheduled_unlock_at } else { None };

    Ok(LockStatusResponse {
        locked,
        unlock_at,
        manually_unlocked,
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
                  p.team_name, p.photo_url, p.price, p.created_at,
                  tp.assigned_position,
                  COALESCE((
                    SELECT SUM(
                      CASE COALESCE(tp.assigned_position, p.position)::text
                        WHEN 'GK'  THEN pp.goals * 10
                        WHEN 'DEF' THEN pp.goals * 6
                        WHEN 'MID' THEN pp.goals * 5
                        WHEN 'FWD' THEN pp.goals * 4
                        ELSE 0
                      END
                      + pp.assists * 5
                      + CASE COALESCE(tp.assigned_position, p.position)::text
                          WHEN 'GK'  THEN pp.clean_sheets * 10
                          WHEN 'DEF' THEN pp.clean_sheets * 6
                          ELSE 0
                        END
                      + CASE WHEN COALESCE(tp.assigned_position, p.position)::text = 'GK' THEN pp.saves / 5 ELSE 0 END
                      + pp.penalty_saves * 8
                      + CASE WHEN pp.minutes_played >= 35 THEN 2
                             WHEN pp.minutes_played >= 1  THEN 1
                             ELSE 0 END
                      - pp.own_goals * 2
                      - pp.penalty_misses * 2
                      - pp.regular_fouls * 1
                      - pp.serious_fouls * 3
                    )
                    FROM player_points pp
                    WHERE pp.player_id = p.id
                  ), 0)::int AS total_points
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

    let total_points = team_total_points(pool, team.id).await?;

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

async fn team_total_points(pool: &sqlx::PgPool, team_id: Uuid) -> Result<i32, AppError> {
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(total_points), 0) FROM team_gameweek_points WHERE team_id = $1",
    )
    .bind(team_id)
    .fetch_one(pool)
    .await?;

    Ok(total as i32)
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
    let lock = compute_lock_status(&state.pool).await?;
    if lock.locked {
        return Err(AppError::BadRequest(
            "Lineup changes are locked from Saturday midnight to Sunday 12:00 PM ET".to_string(),
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
    let _team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    // When a gameweek is active, only allow rearranging existing squad (no new players).
    // To bring in new players, use the transfer endpoint.
    let has_active_week =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM match_weeks WHERE is_active = true")
            .fetch_one(&state.pool)
            .await?
            > 0;

    let current_player_ids =
        sqlx::query_scalar::<_, Uuid>("SELECT player_id FROM team_players WHERE team_id = $1")
            .bind(team_id)
            .fetch_all(&state.pool)
            .await?;

    if has_active_week && !current_player_ids.is_empty() {
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
                  p.team_name, p.photo_url, p.price, p.created_at,
                  tp.assigned_position,
                  COALESCE((
                    SELECT SUM(
                      CASE COALESCE(tp.assigned_position, p.position)::text
                        WHEN 'GK'  THEN pp.goals * 10
                        WHEN 'DEF' THEN pp.goals * 6
                        WHEN 'MID' THEN pp.goals * 5
                        WHEN 'FWD' THEN pp.goals * 4
                        ELSE 0
                      END
                      + pp.assists * 5
                      + CASE COALESCE(tp.assigned_position, p.position)::text
                          WHEN 'GK'  THEN pp.clean_sheets * 10
                          WHEN 'DEF' THEN pp.clean_sheets * 6
                          ELSE 0
                        END
                      + CASE WHEN COALESCE(tp.assigned_position, p.position)::text = 'GK' THEN pp.saves / 5 ELSE 0 END
                      + pp.penalty_saves * 8
                      + CASE WHEN pp.minutes_played >= 35 THEN 2
                             WHEN pp.minutes_played >= 1  THEN 1
                             ELSE 0 END
                      - pp.own_goals * 2
                      - pp.penalty_misses * 2
                      - pp.regular_fouls * 1
                      - pp.serious_fouls * 3
                    )
                    FROM player_points pp
                    WHERE pp.player_id = p.id
                  ), 0)::int AS total_points
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
        "SELECT id, user_id, name, captain_id, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
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
            "Transfers are locked from Saturday midnight to Sunday 12:00 PM ET".to_string(),
        ));
    }

    let team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
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

    let budget_limit = rust_decimal::Decimal::from(70);
    if total_cost > budget_limit {
        return Err(AppError::BadRequest(format!(
            "Transfer would push team cost to ${total_cost}, exceeding the $70 budget"
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
        "SELECT id, user_id, name, captain_id, created_at FROM fantasy_teams WHERE id = $1",
    )
    .bind(team_id)
    .fetch_one(&state.pool)
    .await?;

    let response = build_team_response(&state.pool, &updated_team).await?;
    Ok(Json(response))
}
