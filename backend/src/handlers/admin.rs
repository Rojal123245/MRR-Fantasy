use axum::{
    extract::{Path, State},
    Json,
};

use crate::auth::handler::AppState;
use crate::error::{AppError, AppResult};
use crate::models::{
    AdminPlayerStats, CreateGameweekRequest, MatchWeek, PlayerStatInput,
};
use crate::services::points_engine::PointsEngine;
use crate::models::PlayerPosition;

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
        let position: PlayerPosition = sqlx::query_scalar(
            "SELECT position FROM players WHERE id = $1",
        )
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

    tx.commit().await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "players_updated": stats.len(),
        "week": week_number
    })))
}
