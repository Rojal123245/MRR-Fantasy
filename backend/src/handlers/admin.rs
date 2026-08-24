use axum::{
    extract::{Path, State},
    Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::error::{AppError, AppResult};
use crate::handlers::teams::compute_lock_status;
use crate::models::PlayerPosition;
use crate::models::{AdminPlayerStats, CreateGameweekRequest, MatchWeek, PlayerStatInput};
use crate::services::points_engine::PointsEngine;
use crate::services::points_sql;
use crate::services::scoring;

fn price_floor() -> Decimal {
    Decimal::new(1, 1) // 0.1
}

fn top_price_deltas() -> [Decimal; 3] {
    [
        Decimal::new(3, 1),
        Decimal::new(2, 1),
        Decimal::new(1, 1),
    ]
}

fn bottom_price_deltas() -> [Decimal; 3] {
    [
        Decimal::new(-3, 1),
        Decimal::new(-2, 1),
        Decimal::new(-1, 1),
    ]
}

/// Top 3 / bottom 3 by this gameweek's `player_points.total_points`; reverses any prior
/// adjustment for the same week when stats are resubmitted.
async fn apply_gameweek_price_adjustments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_week_id: Uuid,
) -> Result<(), sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct PrevDelta {
        player_id: Uuid,
        delta: Decimal,
    }

    let prev: Vec<PrevDelta> = sqlx::query_as(
        "SELECT player_id, delta FROM gameweek_price_adjustments WHERE match_week_id = $1",
    )
    .bind(match_week_id)
    .fetch_all(&mut **tx)
    .await?;

    for row in prev {
        sqlx::query(
            "UPDATE players SET price = GREATEST(price - $1, $2) WHERE id = $3",
        )
        .bind(row.delta)
        .bind(price_floor())
        .bind(row.player_id)
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query("DELETE FROM gameweek_price_adjustments WHERE match_week_id = $1")
        .bind(match_week_id)
        .execute(&mut **tx)
        .await?;

    #[derive(sqlx::FromRow)]
    struct PlayerGwRow {
        id: Uuid,
    }

    let ordered: Vec<PlayerGwRow> = sqlx::query_as(
        r#"SELECT p.id
           FROM players p
           LEFT JOIN player_points pp ON pp.player_id = p.id AND pp.match_week_id = $1
           ORDER BY COALESCE(pp.total_points, 0) DESC, p.name ASC"#,
    )
    .bind(match_week_id)
    .fetch_all(&mut **tx)
    .await?;

    let top_ids: Vec<Uuid> = ordered.iter().take(3).map(|r| r.id).collect();

    let mut bottom_ids: Vec<Uuid> = Vec::new();
    for r in ordered.iter().rev() {
        if bottom_ids.len() >= 3 {
            break;
        }
        if !top_ids.contains(&r.id) {
            bottom_ids.push(r.id);
        }
    }

    async fn apply_one(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        match_week_id: Uuid,
        player_id: Uuid,
        intended: Decimal,
    ) -> Result<(), sqlx::Error> {
        let current: Decimal =
            sqlx::query_scalar("SELECT price FROM players WHERE id = $1")
                .bind(player_id)
                .fetch_one(&mut **tx)
                .await?;
        let new_price = (current + intended).max(price_floor());
        let actual = new_price - current;
        if actual.is_zero() {
            return Ok(());
        }
        sqlx::query("UPDATE players SET price = $1 WHERE id = $2")
            .bind(new_price)
            .bind(player_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query(
            r#"INSERT INTO gameweek_price_adjustments (match_week_id, player_id, delta)
               VALUES ($1, $2, $3)"#,
        )
        .bind(match_week_id)
        .bind(player_id)
        .bind(actual)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    let top_d = top_price_deltas();
    for (i, &pid) in top_ids.iter().enumerate() {
        if i < top_d.len() {
            apply_one(tx, match_week_id, pid, top_d[i]).await?;
        }
    }
    let bot_d = bottom_price_deltas();
    for (i, &pid) in bottom_ids.iter().enumerate() {
        if i < bot_d.len() {
            apply_one(tx, match_week_id, pid, bot_d[i]).await?;
        }
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

    scoring::snapshot_all_lineups(&mut tx, week.id).await?;

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
        scoring::snapshot_all_lineups(&mut tx, updated.id).await?;
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

    apply_gameweek_price_adjustments(&mut tx, week.id).await?;

    // Carry each user's team value forward as their next budget limit.
    // This makes budget changes from price movements user-specific.
    sqlx::query(
        r#"UPDATE fantasy_teams ft
           SET budget_limit = team_cost.total_cost
           FROM (
             SELECT tp.team_id, COALESCE(SUM(p.price), 0) AS total_cost
             FROM team_players tp
             JOIN players p ON p.id = tp.player_id
             GROUP BY tp.team_id
           ) AS team_cost
           WHERE ft.id = team_cost.team_id"#,
    )
    .execute(&mut *tx)
    .await?;

    #[derive(sqlx::FromRow)]
    struct TeamScoreContext {
        id: Uuid,
        lineup_id: Option<Uuid>,
        captain_id: Option<Uuid>,
    }

    let total_teams = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM fantasy_teams")
        .fetch_one(&mut *tx)
        .await?;

    let teams = sqlx::query_as::<_, TeamScoreContext>(&points_sql::scored_teams())
        .bind(week.id)
        .bind(week.end_date)
        .fetch_all(&mut *tx)
        .await?;

    let teams_scored = teams.len() as i64;

    for team in teams {
        // A snapshot, once taken, is the source of truth for that week so later
        // transfers cannot change an already-scored gameweek.
        let score = scoring::score_team_gameweek(
            &mut tx,
            team.id,
            team.lineup_id,
            team.captain_id,
            week.id,
        )
        .await?;

        scoring::store_team_gameweek_score(&mut tx, team.id, week.id, &score).await?;
    }

    // Roll the league on: a scored week is over, and the next one opens in the
    // same breath. Leaving the scored week active let chips, transfers and
    // lineup changes keep landing on a week whose points were already stored,
    // where they could no longer affect anything; closing it without opening a
    // successor would leave managers with nowhere to play.
    //
    // Only when the week being scored is the live one. Re-running an older
    // gameweek to correct it must not wind the league back to that point.
    let opened = scoring::close_week_and_open_next(
        &mut tx,
        week.id,
        week.week_number,
        week.is_active,
    )
    .await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "players_updated": stats.len(),
        "week": week_number,
        "teams_scored": teams_scored,
        // Teams that had not joined by the end of this week, so they are left out
        // rather than scored against their current squad.
        "teams_skipped": total_teams - teams_scored,
        // What the league did as a result: the scored week closed, and the next
        // one opened with every squad frozen into it. Null when an older week
        // was re-scored, or when that was the final gameweek.
        "gameweek_closed": if week.is_active { Some(week.week_number) } else { None },
        "gameweek_opened": opened,
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
