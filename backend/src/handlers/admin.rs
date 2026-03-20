use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::error::{AppError, AppResult};
use crate::handlers::teams::compute_lock_status;
use crate::models::PlayerPosition;
use crate::models::{AdminPlayerStats, CreateGameweekRequest, MatchWeek, PlayerStatInput};
use crate::services::points_engine::PointsEngine;

#[derive(sqlx::FromRow)]
struct TeamLineupSnapshotSource {
    id: Uuid,
    captain_id: Option<Uuid>,
}

async fn snapshot_lineups_for_week(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_week_id: Uuid,
) -> Result<(), sqlx::Error> {
    let teams = sqlx::query_as::<_, TeamLineupSnapshotSource>(
        "SELECT id, captain_id FROM fantasy_teams",
    )
    .fetch_all(&mut **tx)
    .await?;

    for team in teams {
        // Keep snapshot immutable once created for this team+week.
        let lineup_id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO team_gameweek_lineups (team_id, match_week_id, captain_id)
               VALUES ($1, $2, $3)
               ON CONFLICT (team_id, match_week_id) DO UPDATE
                 SET captain_id = team_gameweek_lineups.captain_id
               RETURNING id"#,
        )
        .bind(team.id)
        .bind(match_week_id)
        .bind(team.captain_id)
        .fetch_one(&mut **tx)
        .await?;

        sqlx::query(
            r#"INSERT INTO team_gameweek_lineup_players
                 (team_gameweek_lineup_id, player_id, is_bench, assigned_position)
               SELECT $1, tp.player_id, tp.is_bench, tp.assigned_position
               FROM team_players tp
               WHERE tp.team_id = $2
               ON CONFLICT (team_gameweek_lineup_id, player_id) DO NOTHING"#,
        )
        .bind(lineup_id)
        .bind(team.id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SetLineupLockRequest {
    pub force_unlock: bool,
}

#[derive(Debug, Serialize)]
pub struct AdminLineupLockResponse {
    pub force_unlock: bool,
    pub effective_locked: bool,
    pub unlock_at: Option<String>,
}

/// POST /api/admin/gameweek
///
/// Create a new match week. Deactivates any previously active week.
pub async fn create_gameweek(
    State(state): State<AppState>,
    Json(body): Json<CreateGameweekRequest>,
) -> AppResult<Json<MatchWeek>> {
    let mut tx = state.pool.begin().await?;

    sqlx::query("UPDATE match_weeks SET is_active = false WHERE is_active = true")
        .execute(&mut *tx)
        .await?;

    let week = sqlx::query_as::<_, MatchWeek>(
        r#"INSERT INTO match_weeks (week_number, start_date, end_date, is_active)
           VALUES ($1, $2, $3, true)
           ON CONFLICT (week_number) DO UPDATE
             SET start_date = EXCLUDED.start_date,
                 end_date = EXCLUDED.end_date,
                 is_active = true
           RETURNING id, week_number, start_date, end_date, is_active"#,
    )
    .bind(body.week_number)
    .bind(body.start_date)
    .bind(body.end_date)
    .fetch_one(&mut *tx)
    .await?;

    snapshot_lineups_for_week(&mut tx, week.id).await?;

    tx.commit().await?;

    Ok(Json(week))
}

/// GET /api/admin/gameweek/:week/stats
///
/// Get all player stats for a given week (zeros if not yet entered).
pub async fn get_week_stats(
    State(state): State<AppState>,
    Path(week_number): Path<i32>,
) -> AppResult<Json<Vec<AdminPlayerStats>>> {
    let stats = sqlx::query_as::<_, AdminPlayerStats>(
        r#"SELECT
             p.id AS player_id,
             p.name AS player_name,
             p.position::text AS position,
             COALESCE(pp.goals, 0) AS goals,
             COALESCE(pp.assists, 0) AS assists,
             COALESCE(pp.clean_sheets, 0) AS clean_sheets,
             COALESCE(pp.saves, 0) AS saves,
             COALESCE(pp.penalty_saves, 0) AS penalty_saves,
             COALESCE(pp.own_goals, 0) AS own_goals,
             COALESCE(pp.penalty_misses, 0) AS penalty_misses,
             COALESCE(pp.regular_fouls, 0) AS regular_fouls,
             COALESCE(pp.serious_fouls, 0) AS serious_fouls,
             COALESCE(pp.minutes_played, 0) AS minutes_played,
             COALESCE(pp.total_points, 0) AS total_points
           FROM players p
           LEFT JOIN player_points pp ON pp.player_id = p.id
             AND pp.match_week_id = (SELECT id FROM match_weeks WHERE week_number = $1)
           ORDER BY p.position, p.name"#,
    )
    .bind(week_number)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(stats))
}

/// GET /api/admin/gameweeks
///
/// List all gameweeks with their status.
pub async fn get_gameweeks(State(state): State<AppState>) -> AppResult<Json<Vec<MatchWeek>>> {
    let weeks = sqlx::query_as::<_, MatchWeek>(
        "SELECT id, week_number, start_date, end_date, is_active FROM match_weeks ORDER BY week_number",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(weeks))
}

/// PUT /api/admin/gameweek/:week/toggle
///
/// Toggle a gameweek's active status. When activating, deactivates all others.
/// When deactivating, simply sets is_active = false (no active gameweek).
pub async fn toggle_gameweek(
    State(state): State<AppState>,
    Path(week_number): Path<i32>,
) -> AppResult<Json<MatchWeek>> {
    let current = sqlx::query_as::<_, MatchWeek>(
        "SELECT id, week_number, start_date, end_date, is_active FROM match_weeks WHERE week_number = $1",
    )
    .bind(week_number)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Gameweek {week_number} not found. Create it first.")))?;

    let mut tx = state.pool.begin().await?;

    if current.is_active {
        sqlx::query("UPDATE match_weeks SET is_active = false WHERE week_number = $1")
            .bind(week_number)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE match_weeks SET is_active = false WHERE is_active = true")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE match_weeks SET is_active = true WHERE week_number = $1")
            .bind(week_number)
            .execute(&mut *tx)
            .await?;
    }

    let updated = sqlx::query_as::<_, MatchWeek>(
        "SELECT id, week_number, start_date, end_date, is_active FROM match_weeks WHERE week_number = $1",
    )
    .bind(week_number)
    .fetch_one(&mut *tx)
    .await?;

    if updated.is_active {
        snapshot_lineups_for_week(&mut tx, updated.id).await?;
    }

    tx.commit().await?;

    Ok(Json(updated))
}

/// POST /api/admin/gameweek/:week/stats
///
/// Batch upsert player stats for a gameweek, recalculate points.
pub async fn submit_week_stats(
    State(state): State<AppState>,
    Path(week_number): Path<i32>,
    Json(stats): Json<Vec<PlayerStatInput>>,
) -> AppResult<Json<serde_json::Value>> {
    let week = sqlx::query_as::<_, MatchWeek>(
        "SELECT id, week_number, start_date, end_date, is_active FROM match_weeks WHERE week_number = $1",
    )
    .bind(week_number)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Gameweek {week_number} not found")))?;

    let mut tx = state.pool.begin().await?;

    for stat in &stats {
        let position: PlayerPosition =
            sqlx::query_scalar("SELECT position FROM players WHERE id = $1")
                .bind(stat.player_id)
                .fetch_one(&mut *tx)
                .await?;

        let total = PointsEngine::calculate(
            &position,
            stat.goals,
            stat.assists,
            stat.clean_sheets,
            stat.saves,
            stat.penalty_saves,
            stat.own_goals,
            stat.penalty_misses,
            stat.regular_fouls,
            stat.serious_fouls,
            stat.minutes_played,
        );

        sqlx::query(
            r#"INSERT INTO player_points
                 (player_id, match_week_id, goals, assists, clean_sheets, saves,
                  penalty_saves, own_goals, penalty_misses, regular_fouls, serious_fouls,
                  minutes_played, total_points)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
               ON CONFLICT (player_id, match_week_id) DO UPDATE SET
                 goals = EXCLUDED.goals,
                 assists = EXCLUDED.assists,
                 clean_sheets = EXCLUDED.clean_sheets,
                 saves = EXCLUDED.saves,
                 penalty_saves = EXCLUDED.penalty_saves,
                 own_goals = EXCLUDED.own_goals,
                 penalty_misses = EXCLUDED.penalty_misses,
                 regular_fouls = EXCLUDED.regular_fouls,
                 serious_fouls = EXCLUDED.serious_fouls,
                 minutes_played = EXCLUDED.minutes_played,
                 total_points = EXCLUDED.total_points"#,
        )
        .bind(stat.player_id)
        .bind(week.id)
        .bind(stat.goals)
        .bind(stat.assists)
        .bind(stat.clean_sheets)
        .bind(stat.saves)
        .bind(stat.penalty_saves)
        .bind(stat.own_goals)
        .bind(stat.penalty_misses)
        .bind(stat.regular_fouls)
        .bind(stat.serious_fouls)
        .bind(stat.minutes_played)
        .bind(total)
        .execute(&mut *tx)
        .await?;
    }

    // Recalculate players.total_points as sum across all weeks (using primary position)
    sqlx::query(
        r#"UPDATE players SET total_points = sub.pts
           FROM (
             SELECT player_id, COALESCE(SUM(total_points), 0)::int AS pts
             FROM player_points
             GROUP BY player_id
           ) sub
           WHERE players.id = sub.player_id"#,
    )
    .execute(&mut *tx)
    .await?;

    #[derive(sqlx::FromRow)]
    struct TeamScoreContext {
        id: Uuid,
        lineup_id: Option<Uuid>,
        captain_id: Option<Uuid>,
    }

    let teams = sqlx::query_as::<_, TeamScoreContext>(
        r#"SELECT
             ft.id,
             tgl.id AS lineup_id,
             COALESCE(tgl.captain_id, ft.captain_id) AS captain_id
           FROM fantasy_teams ft
           LEFT JOIN team_gameweek_lineups tgl
             ON tgl.team_id = ft.id AND tgl.match_week_id = $1"#,
    )
    .bind(week.id)
    .fetch_all(&mut *tx)
    .await?;

    for team in teams {
        let starter_base = if let Some(lineup_id) = team.lineup_id {
            sqlx::query_scalar::<_, i64>(
                r#"SELECT COALESCE(SUM(
                     CASE COALESCE(tglp.assigned_position, p.position)::text
                       WHEN 'GK'  THEN COALESCE(pp.goals, 0) * 10
                       WHEN 'DEF' THEN COALESCE(pp.goals, 0) * 6
                       WHEN 'MID' THEN COALESCE(pp.goals, 0) * 5
                       WHEN 'FWD' THEN COALESCE(pp.goals, 0) * 4
                       ELSE 0
                     END
                     + COALESCE(pp.assists, 0) * 5
                     + CASE COALESCE(tglp.assigned_position, p.position)::text
                         WHEN 'GK'  THEN COALESCE(pp.clean_sheets, 0) * 10
                         WHEN 'DEF' THEN COALESCE(pp.clean_sheets, 0) * 6
                         ELSE 0
                       END
                     + CASE WHEN COALESCE(tglp.assigned_position, p.position)::text = 'GK' THEN COALESCE(pp.saves, 0) / 5 ELSE 0 END
                     + COALESCE(pp.penalty_saves, 0) * 8
                     + CASE WHEN COALESCE(pp.minutes_played, 0) >= 35 THEN 2
                            WHEN COALESCE(pp.minutes_played, 0) >= 1  THEN 1
                            ELSE 0 END
                     - COALESCE(pp.own_goals, 0) * 2
                     - COALESCE(pp.penalty_misses, 0) * 2
                     - COALESCE(pp.regular_fouls, 0) * 1
                     - COALESCE(pp.serious_fouls, 0) * 3
                   ), 0)
                   FROM team_gameweek_lineup_players tglp
                   JOIN players p ON p.id = tglp.player_id
                   LEFT JOIN player_points pp ON pp.player_id = tglp.player_id AND pp.match_week_id = $2
                   WHERE tglp.team_gameweek_lineup_id = $1 AND tglp.is_bench = false"#,
            )
            .bind(lineup_id)
            .bind(week.id)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_scalar::<_, i64>(
                r#"SELECT COALESCE(SUM(
                     CASE COALESCE(tp.assigned_position, p.position)::text
                       WHEN 'GK'  THEN COALESCE(pp.goals, 0) * 10
                       WHEN 'DEF' THEN COALESCE(pp.goals, 0) * 6
                       WHEN 'MID' THEN COALESCE(pp.goals, 0) * 5
                       WHEN 'FWD' THEN COALESCE(pp.goals, 0) * 4
                       ELSE 0
                     END
                     + COALESCE(pp.assists, 0) * 5
                     + CASE COALESCE(tp.assigned_position, p.position)::text
                         WHEN 'GK'  THEN COALESCE(pp.clean_sheets, 0) * 10
                         WHEN 'DEF' THEN COALESCE(pp.clean_sheets, 0) * 6
                         ELSE 0
                       END
                     + CASE WHEN COALESCE(tp.assigned_position, p.position)::text = 'GK' THEN COALESCE(pp.saves, 0) / 5 ELSE 0 END
                     + COALESCE(pp.penalty_saves, 0) * 8
                     + CASE WHEN COALESCE(pp.minutes_played, 0) >= 35 THEN 2
                            WHEN COALESCE(pp.minutes_played, 0) >= 1  THEN 1
                            ELSE 0 END
                     - COALESCE(pp.own_goals, 0) * 2
                     - COALESCE(pp.penalty_misses, 0) * 2
                     - COALESCE(pp.regular_fouls, 0) * 1
                     - COALESCE(pp.serious_fouls, 0) * 3
                   ), 0)
                   FROM team_players tp
                   JOIN players p ON p.id = tp.player_id
                   LEFT JOIN player_points pp ON pp.player_id = tp.player_id AND pp.match_week_id = $2
                   WHERE tp.team_id = $1 AND tp.is_bench = false"#,
            )
            .bind(team.id)
            .bind(week.id)
            .fetch_one(&mut *tx)
            .await?
        };

        let triple_captain_active = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM team_chips WHERE team_id = $1 AND match_week_id = $2 AND chip_type = 'triple_captain'",
        )
        .bind(team.id)
        .bind(week.id)
        .fetch_one(&mut *tx)
        .await?
            > 0;

        let bench_boost_active = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM team_chips WHERE team_id = $1 AND match_week_id = $2 AND chip_type = 'bench_boost'",
        )
        .bind(team.id)
        .bind(week.id)
        .fetch_one(&mut *tx)
        .await?
            > 0;

        let captain_bonus = if let Some(captain_id) = team.captain_id {
            let captain_points = if let Some(lineup_id) = team.lineup_id {
                sqlx::query_scalar::<_, i32>(
                    r#"SELECT COALESCE((
                         CASE COALESCE(tglp.assigned_position, p.position)::text
                           WHEN 'GK'  THEN COALESCE(pp.goals, 0) * 10
                           WHEN 'DEF' THEN COALESCE(pp.goals, 0) * 6
                           WHEN 'MID' THEN COALESCE(pp.goals, 0) * 5
                           WHEN 'FWD' THEN COALESCE(pp.goals, 0) * 4
                           ELSE 0
                         END
                         + COALESCE(pp.assists, 0) * 5
                         + CASE COALESCE(tglp.assigned_position, p.position)::text
                             WHEN 'GK'  THEN COALESCE(pp.clean_sheets, 0) * 10
                             WHEN 'DEF' THEN COALESCE(pp.clean_sheets, 0) * 6
                             ELSE 0
                           END
                         + CASE WHEN COALESCE(tglp.assigned_position, p.position)::text = 'GK' THEN COALESCE(pp.saves, 0) / 5 ELSE 0 END
                         + COALESCE(pp.penalty_saves, 0) * 8
                         + CASE WHEN COALESCE(pp.minutes_played, 0) >= 35 THEN 2
                                WHEN COALESCE(pp.minutes_played, 0) >= 1  THEN 1
                                ELSE 0 END
                         - COALESCE(pp.own_goals, 0) * 2
                         - COALESCE(pp.penalty_misses, 0) * 2
                         - COALESCE(pp.regular_fouls, 0) * 1
                         - COALESCE(pp.serious_fouls, 0) * 3
                       ), 0)
                       FROM team_gameweek_lineup_players tglp
                       JOIN players p ON p.id = tglp.player_id
                       LEFT JOIN player_points pp ON pp.player_id = tglp.player_id AND pp.match_week_id = $2
                       WHERE tglp.team_gameweek_lineup_id = $1
                         AND tglp.player_id = $3
                         AND tglp.is_bench = false"#,
                )
                .bind(lineup_id)
                .bind(week.id)
                .bind(captain_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0)
            } else {
                sqlx::query_scalar::<_, i32>(
                    r#"SELECT COALESCE((
                         CASE COALESCE(tp.assigned_position, p.position)::text
                           WHEN 'GK'  THEN COALESCE(pp.goals, 0) * 10
                           WHEN 'DEF' THEN COALESCE(pp.goals, 0) * 6
                           WHEN 'MID' THEN COALESCE(pp.goals, 0) * 5
                           WHEN 'FWD' THEN COALESCE(pp.goals, 0) * 4
                           ELSE 0
                         END
                         + COALESCE(pp.assists, 0) * 5
                         + CASE COALESCE(tp.assigned_position, p.position)::text
                             WHEN 'GK'  THEN COALESCE(pp.clean_sheets, 0) * 10
                             WHEN 'DEF' THEN COALESCE(pp.clean_sheets, 0) * 6
                             ELSE 0
                           END
                         + CASE WHEN COALESCE(tp.assigned_position, p.position)::text = 'GK' THEN COALESCE(pp.saves, 0) / 5 ELSE 0 END
                         + COALESCE(pp.penalty_saves, 0) * 8
                         + CASE WHEN COALESCE(pp.minutes_played, 0) >= 35 THEN 2
                                WHEN COALESCE(pp.minutes_played, 0) >= 1  THEN 1
                                ELSE 0 END
                         - COALESCE(pp.own_goals, 0) * 2
                         - COALESCE(pp.penalty_misses, 0) * 2
                         - COALESCE(pp.regular_fouls, 0) * 1
                         - COALESCE(pp.serious_fouls, 0) * 3
                       ), 0)
                       FROM fantasy_teams ft
                       JOIN team_players tp ON tp.team_id = ft.id AND tp.player_id = ft.captain_id AND tp.is_bench = false
                       JOIN players p ON p.id = tp.player_id
                       LEFT JOIN player_points pp ON pp.player_id = tp.player_id AND pp.match_week_id = $2
                       WHERE ft.id = $1 AND ft.captain_id = $3"#,
                )
                .bind(team.id)
                .bind(week.id)
                .bind(captain_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0)
            };

            if triple_captain_active {
                (captain_points * 2) as i64
            } else {
                captain_points as i64
            }
        } else {
            0
        };

        let bench_bonus = if bench_boost_active {
            if let Some(lineup_id) = team.lineup_id {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT COALESCE(SUM(
                         CASE p.position::text
                           WHEN 'GK'  THEN COALESCE(pp.goals, 0) * 10
                           WHEN 'DEF' THEN COALESCE(pp.goals, 0) * 6
                           WHEN 'MID' THEN COALESCE(pp.goals, 0) * 5
                           WHEN 'FWD' THEN COALESCE(pp.goals, 0) * 4
                           ELSE 0
                         END
                         + COALESCE(pp.assists, 0) * 5
                         + CASE p.position::text
                             WHEN 'GK'  THEN COALESCE(pp.clean_sheets, 0) * 10
                             WHEN 'DEF' THEN COALESCE(pp.clean_sheets, 0) * 6
                             ELSE 0
                           END
                         + CASE WHEN p.position::text = 'GK' THEN COALESCE(pp.saves, 0) / 5 ELSE 0 END
                         + COALESCE(pp.penalty_saves, 0) * 8
                         + CASE WHEN COALESCE(pp.minutes_played, 0) >= 35 THEN 2
                                WHEN COALESCE(pp.minutes_played, 0) >= 1  THEN 1
                                ELSE 0 END
                         - COALESCE(pp.own_goals, 0) * 2
                         - COALESCE(pp.penalty_misses, 0) * 2
                         - COALESCE(pp.regular_fouls, 0) * 1
                         - COALESCE(pp.serious_fouls, 0) * 3
                       ), 0)
                       FROM team_gameweek_lineup_players tglp
                       JOIN players p ON p.id = tglp.player_id
                       LEFT JOIN player_points pp ON pp.player_id = tglp.player_id AND pp.match_week_id = $2
                       WHERE tglp.team_gameweek_lineup_id = $1 AND tglp.is_bench = true"#,
                )
                .bind(lineup_id)
                .bind(week.id)
                .fetch_one(&mut *tx)
                .await?
            } else {
                sqlx::query_scalar::<_, i64>(
                    r#"SELECT COALESCE(SUM(
                         CASE p.position::text
                           WHEN 'GK'  THEN COALESCE(pp.goals, 0) * 10
                           WHEN 'DEF' THEN COALESCE(pp.goals, 0) * 6
                           WHEN 'MID' THEN COALESCE(pp.goals, 0) * 5
                           WHEN 'FWD' THEN COALESCE(pp.goals, 0) * 4
                           ELSE 0
                         END
                         + COALESCE(pp.assists, 0) * 5
                         + CASE p.position::text
                             WHEN 'GK'  THEN COALESCE(pp.clean_sheets, 0) * 10
                             WHEN 'DEF' THEN COALESCE(pp.clean_sheets, 0) * 6
                             ELSE 0
                           END
                         + CASE WHEN p.position::text = 'GK' THEN COALESCE(pp.saves, 0) / 5 ELSE 0 END
                         + COALESCE(pp.penalty_saves, 0) * 8
                         + CASE WHEN COALESCE(pp.minutes_played, 0) >= 35 THEN 2
                                WHEN COALESCE(pp.minutes_played, 0) >= 1  THEN 1
                                ELSE 0 END
                         - COALESCE(pp.own_goals, 0) * 2
                         - COALESCE(pp.penalty_misses, 0) * 2
                         - COALESCE(pp.regular_fouls, 0) * 1
                         - COALESCE(pp.serious_fouls, 0) * 3
                       ), 0)
                       FROM team_players tp
                       JOIN players p ON p.id = tp.player_id
                       LEFT JOIN player_points pp ON pp.player_id = tp.player_id AND pp.match_week_id = $2
                       WHERE tp.team_id = $1 AND tp.is_bench = true"#,
                )
                .bind(team.id)
                .bind(week.id)
                .fetch_one(&mut *tx)
                .await?
            }
        } else {
            0
        };

        let transfers_this_week = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transfers WHERE team_id = $1 AND match_week_id = $2",
        )
        .bind(team.id)
        .bind(week.id)
        .fetch_one(&mut *tx)
        .await?;

        let transfer_points_hit = ((transfers_this_week as i32) - 1).max(0) * 4;
        let gross_points = (starter_base + captain_bonus + bench_bonus) as i32;
        let total_points = gross_points - transfer_points_hit;

        sqlx::query(
            r#"INSERT INTO team_gameweek_points
                 (team_id, match_week_id, gross_points, transfer_points_hit, total_points)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (team_id, match_week_id) DO UPDATE SET
                 gross_points = EXCLUDED.gross_points,
                 transfer_points_hit = EXCLUDED.transfer_points_hit,
                 total_points = EXCLUDED.total_points,
                 updated_at = NOW()"#,
        )
        .bind(team.id)
        .bind(week.id)
        .bind(gross_points)
        .bind(transfer_points_hit)
        .bind(total_points)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "players_updated": stats.len(),
        "week": week_number
    })))
}

/// GET /api/admin/lineup-lock
///
/// Returns the current lineup lock override and effective lock status.
pub async fn get_lineup_lock_control(
    State(state): State<AppState>,
) -> AppResult<Json<AdminLineupLockResponse>> {
    let force_unlock = sqlx::query_scalar::<_, bool>(
        "SELECT force_unlock FROM lineup_lock_control WHERE id = true",
    )
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(false);

    let lock = compute_lock_status(&state.pool).await?;
    Ok(Json(AdminLineupLockResponse {
        force_unlock,
        effective_locked: lock.locked,
        unlock_at: lock.unlock_at,
    }))
}

/// PUT /api/admin/lineup-lock
///
/// Allows admins to manually unlock/restore the scheduled weekend lock.
pub async fn set_lineup_lock_control(
    State(state): State<AppState>,
    Json(body): Json<SetLineupLockRequest>,
) -> AppResult<Json<AdminLineupLockResponse>> {
    sqlx::query(
        r#"INSERT INTO lineup_lock_control (id, force_unlock)
           VALUES (true, $1)
           ON CONFLICT (id) DO UPDATE SET
             force_unlock = EXCLUDED.force_unlock,
             updated_at = NOW()"#,
    )
    .bind(body.force_unlock)
    .execute(&state.pool)
    .await?;

    let lock = compute_lock_status(&state.pool).await?;
    Ok(Json(AdminLineupLockResponse {
        force_unlock: body.force_unlock,
        effective_locked: lock.locked,
        unlock_at: lock.unlock_at,
    }))
}
