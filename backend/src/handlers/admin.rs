use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::error::{AppError, AppResult};
use crate::handlers::teams::{self, compute_lock_status};
use crate::models::PlayerPosition;
use crate::models::{AdminPlayerStats, CreateGameweekRequest, MatchWeek, PlayerStatInput};
use crate::services::deadline;
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

/// Top 3 / bottom 3 by this gameweek's `player_points.total_points`, written to
/// `players.price` and recorded in `gameweek_price_adjustments` so that scoring
/// the week again does not double-count.
///
/// Re-scoring week N rebuilds every recorded week from N forward, not just N.
///
/// The reversal has to be exact: `delta` is already the post-clamp amount that
/// actually landed, so its true inverse is a plain subtraction. Subtracting it
/// is only safe once nothing later sits on top of the price, though. A player
/// lifted in week 5 and driven to the $0.10 floor by week 7 has no room left to
/// give week 5's rise back, and clamping the reversal instead — what this used
/// to do — kept the difference: the ledger row was deleted regardless and the
/// fresh move applied on top, leaving the player permanently dearer than the
/// sum of their recorded moves, and every squad holding them richer by the
/// swallowed amount, with no price move to justify it.
///
/// So the chain is unwound from the latest recorded week back to N and then
/// re-applied forward from N. Reversing in exact application order retraces the
/// prices the weeks actually passed through, all of which were at or above the
/// floor, and each week's clamp is then recomputed against the price the week
/// before it leaves behind — which is where a clamp belongs.
///
/// A later week is re-derived from its own `player_points`, so it picks the same
/// six players again unless the roster itself has changed since — the same
/// re-derivation week N has always been subject to.
///
/// Weeks scored before this ledger existed have no rows and so are not part of
/// any chain; their prices stay where they are.
async fn apply_gameweek_price_adjustments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_week_id: Uuid,
) -> Result<(), AppError> {
    #[derive(sqlx::FromRow)]
    struct ChainWeek {
        id: Uuid,
        week_number: i32,
    }

    let week_number: i32 = sqlx::query_scalar("SELECT week_number FROM match_weeks WHERE id = $1")
        .bind(match_week_id)
        .fetch_one(&mut **tx)
        .await?;

    // Every week whose moves are stacked on top of this one's, this week
    // included. The ledger records no application order, so week order stands
    // in for it; that is the order `submit_week_stats` produces unless weeks
    // were scored out of sequence.
    let mut chain: Vec<ChainWeek> = sqlx::query_as(
        r#"SELECT DISTINCT w.id, w.week_number
           FROM gameweek_price_adjustments a
           JOIN match_weeks w ON w.id = a.match_week_id
           WHERE w.week_number >= $1
           ORDER BY w.week_number"#,
    )
    .bind(week_number)
    .fetch_all(&mut **tx)
    .await?;

    // Scoring this week for the first time: it has nothing to reverse, but it
    // still leads the chain.
    if !chain.iter().any(|w| w.id == match_week_id) {
        chain.insert(0, ChainWeek { id: match_week_id, week_number });
    }

    for w in chain.iter().rev() {
        reverse_week_price_adjustments(tx, w.id, w.week_number).await?;
    }
    for w in &chain {
        apply_week_price_adjustments(tx, w.id).await?;
    }

    Ok(())
}

/// Undo one week's recorded price moves and drop its ledger rows.
///
/// A price that lands below the floor means it moved outside this ledger — an
/// edit made by hand, or weeks scored out of order — and the chain can no
/// longer be rebuilt from what was recorded. That is refused rather than
/// absorbed: absorbing it is the defect this replaced, and it is silent.
async fn reverse_week_price_adjustments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    week_id: Uuid,
    week_number: i32,
) -> Result<(), AppError> {
    #[derive(sqlx::FromRow)]
    struct PrevDelta {
        player_id: Uuid,
        delta: Decimal,
    }

    let prev: Vec<PrevDelta> = sqlx::query_as(
        "SELECT player_id, delta FROM gameweek_price_adjustments WHERE match_week_id = $1",
    )
    .bind(week_id)
    .fetch_all(&mut **tx)
    .await?;

    for row in prev {
        let (name, price): (String, Decimal) = sqlx::query_as(
            "UPDATE players SET price = price - $1 WHERE id = $2 RETURNING name, price",
        )
        .bind(row.delta)
        .bind(row.player_id)
        .fetch_one(&mut **tx)
        .await?;

        if price < price_floor() {
            return Err(AppError::Conflict(format!(
                "cannot re-score: undoing gameweek {week_number}'s {} move on {name} leaves \
                 {price}, below the {} floor. Their price has been changed outside \
                 gameweek_price_adjustments, so it can no longer be rebuilt from it",
                row.delta,
                price_floor(),
            )));
        }
    }

    sqlx::query("DELETE FROM gameweek_price_adjustments WHERE match_week_id = $1")
        .bind(week_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

/// Move this week's top three up and bottom three down, recording what actually
/// landed after the floor is applied.
async fn apply_week_price_adjustments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_week_id: Uuid,
) -> Result<(), sqlx::Error> {
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

/// What every team's squad is worth right now, one row per team that holds
/// players. Taken either side of a week's price moves to see what those moves
/// did to each manager.
async fn squad_costs(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<HashMap<Uuid, Decimal>, sqlx::Error> {
    let rows: Vec<(Uuid, Decimal)> = sqlx::query_as(
        r#"SELECT tp.team_id, COALESCE(SUM(p.price), 0)
           FROM team_players tp
           JOIN players p ON p.id = tp.player_id
           GROUP BY tp.team_id"#,
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows.into_iter().collect())
}

/// Apply a gameweek's price moves, and carry every manager's spending power
/// along with them.
///
/// A budget is spending power, not squad value. Scoring used to end by setting
/// each budget *to* the squad's value, which confiscated every dollar a manager
/// had deliberately not spent: hold a $68.00 squad against a $70.00 budget and
/// the week ended with a $68.00 budget, the $2.00 saved towards a transfer
/// gone. Managers were charged for keeping money in the bank, and the loss
/// ratcheted — each downgrade freed cash that the next scoring run swallowed,
/// so a squad once affordable could never be afforded again.
///
/// Moving each budget *by* what the week did to that manager's squad leaves the
/// gap between the two untouched. The bank is where its manager left it, while
/// a price rise still buys more and a fall still buys less.
///
/// Measuring the move as a before-and-after of squad value, rather than by
/// summing this week's price deltas, is also what makes re-scoring a gameweek
/// safe. `apply_gameweek_price_adjustments` reverses the week's previous moves
/// before re-applying them, so resubmitting unchanged stats leaves every price
/// where it was: both snapshots agree, no budget moves, and a correction to an
/// old week stops disturbing the money managers are holding in the live one.
async fn apply_gameweek_price_and_budget_changes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_week_id: Uuid,
) -> Result<(), sqlx::Error> {
    let before = squad_costs(tx).await?;
    apply_gameweek_price_adjustments(tx, match_week_id).await?;
    let after = squad_costs(tx).await?;

    // Squads are read either side of one price change and nothing in this
    // transaction moves a player between teams, so the two snapshots cover the
    // same teams holding the same players. A team whose squad did not move in
    // price contributes nothing and keeps the budget it had.
    //
    // A team seen only in `after` would therefore be impossible — but treating
    // its missing `before` as zero would read the squad's entire value as a
    // gain and hand its manager tens of dollars. Skipping is the safe reading
    // of a state that cannot arise.
    let mut team_ids: Vec<Uuid> = Vec::new();
    let mut deltas: Vec<Decimal> = Vec::new();
    for (team_id, after_cost) in &after {
        let Some(before_cost) = before.get(team_id) else {
            continue;
        };
        let delta = after_cost - before_cost;
        if !delta.is_zero() {
            team_ids.push(*team_id);
            deltas.push(delta);
        }
    }

    if team_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        r#"UPDATE fantasy_teams ft
           SET budget_limit = ft.budget_limit + moved.delta
           FROM UNNEST($1::uuid[], $2::numeric[]) AS moved(team_id, delta)
           WHERE ft.id = moved.team_id"#,
    )
    .bind(&team_ids)
    .bind(&deltas)
    .execute(&mut **tx)
    .await?;

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

    // Creating a gameweek means it starts now, so its deadline is set outright.
    scoring::open_week(&mut tx, week.id, Some(deadline::next_deadline())).await?;

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
        // Activating an existing week never moves a deadline it already has.
        scoring::open_week(&mut tx, current.id, None).await?;
    }

    let updated = sqlx::query_as::<_, MatchWeek>(
        "SELECT id, week_number, start_date, end_date, is_active FROM match_weeks WHERE week_number = $1",
    )
    .bind(week_number)
    .fetch_one(&mut *tx)
    .await?;

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

    apply_gameweek_price_and_budget_changes(&mut tx, week.id).await?;

    // Take the week for scoring, and hold it until this transaction ends.
    //
    // Scoring reads each team's lineup across several statements, and under
    // READ COMMITTED each one sees a fresh snapshot. A manager's save
    // committing partway through would be half-counted: starters from the
    // lineup before it, the bench and captain from after, and a transfer hit
    // for a swap that is only in one of them. The stored score would then
    // reproduce from no lineup at all, which is the state
    // `ops/2026-08-24_repair_scored_weeks.sql` existed to clear.
    //
    // The already-scored guard in `refresh_team_lineup` cannot cover this on
    // its own: the rows this transaction is writing to `team_gameweek_points`
    // are invisible to it until commit.
    //
    // Taken here, *after* the `fantasy_teams` writes in
    // `apply_gameweek_price_and_budget_changes` above, so every transaction
    // that touches both takes `fantasy_teams` first and no cycle can form with
    // a save holding a team's row and waiting on the week.
    scoring::lock_week_for_scoring(&mut tx, week.id).await?;

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

    // Reopening the Sunday window has to move the deadline too, or it reopens
    // nothing that counts: the live gameweek would still be sealed, and every
    // save an admin has just invited would be silently dropped — the exact
    // failure the deadline exists to remove.
    //
    // The new deadline is the moment the window would have closed anyway, so
    // the override grants exactly the time it appears to and never extends a
    // gameweek past the noon it was already going to reopen at. GREATEST keeps
    // it monotonic, and a scored week is left alone.
    if body.force_unlock {
        if let Some(window_end) = teams::lock_window_end() {
            sqlx::query(
                r#"UPDATE match_weeks w
                   SET lineup_deadline = GREATEST(w.lineup_deadline, $1)
                   WHERE w.is_active
                     AND NOT EXISTS (
                       SELECT 1 FROM team_gameweek_points g WHERE g.match_week_id = w.id
                     )"#,
            )
            .bind(window_end)
            .execute(&state.pool)
            .await?;
        }
    }

    let lock = compute_lock_status(&state.pool).await?;
    Ok(Json(AdminLineupLockResponse {
        force_unlock: body.force_unlock,
        effective_locked: lock.locked,
        unlock_at: lock.unlock_at,
    }))
}

#[cfg(test)]
mod budget_carry_forward_tests {
    use super::*;

    async fn pool() -> Option<sqlx::PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        sqlx::PgPool::connect(&url).await.ok()
    }

    /// Dollars and cents, as the column stores them.
    fn money(cents: i64) -> Decimal {
        Decimal::new(cents, 2)
    }

    /// A manager holding nine players, and the week whose prices are about to
    /// move. The squad is worth `9 × player_price` against a `budget` the caller
    /// picks: the gap between the two is the bank these tests are about.
    ///
    /// Week numbers sit in a range no real gameweek uses, and every test rolls
    /// its transaction back.
    struct Fixture {
        week_id: Uuid,
        team_id: Uuid,
        player_ids: Vec<Uuid>,
    }

    async fn seed(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        week_number: i32,
        tag: &str,
        budget: Decimal,
        player_price: Decimal,
    ) -> Fixture {
        let week_id: Uuid = sqlx::query_scalar(
            "INSERT INTO match_weeks (week_number, start_date, end_date, is_active)
             VALUES ($1, '2099-01-05'::date, '2099-01-11'::date, false) RETURNING id",
        )
        .bind(week_number)
        .fetch_one(&mut **tx)
        .await
        .expect("insert week");

        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (username, email, password_hash, full_name)
             VALUES ($1, $2, 'x', 'Budget Probe') RETURNING id",
        )
        .bind(format!("budget_probe_{tag}"))
        .bind(format!("budget_probe_{tag}@example.test"))
        .fetch_one(&mut **tx)
        .await
        .expect("insert user");

        let team_id: Uuid = sqlx::query_scalar(
            "INSERT INTO fantasy_teams (user_id, name, budget_limit)
             VALUES ($1, 'Budget FC', $2) RETURNING id",
        )
        .bind(user_id)
        .bind(budget)
        .fetch_one(&mut **tx)
        .await
        .expect("insert team");

        let mut player_ids = Vec::new();
        for i in 0..9 {
            let player_id = add_player(tx, &format!("Budget Probe {tag} {i}"), player_price).await;
            sqlx::query(
                "INSERT INTO team_players (team_id, player_id, is_bench) VALUES ($1, $2, $3)",
            )
            .bind(team_id)
            .bind(player_id)
            .bind(i >= 6)
            .execute(&mut **tx)
            .await
            .expect("insert team player");
            player_ids.push(player_id);
        }

        Fixture {
            week_id,
            team_id,
            player_ids,
        }
    }

    async fn add_player(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        name: &str,
        price: Decimal,
    ) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO players (name, position, team_name, price)
             VALUES ($1, 'MID', 'Probe United', $2) RETURNING id",
        )
        .bind(name)
        .bind(price)
        .fetch_one(&mut **tx)
        .await
        .expect("insert player")
    }

    /// Score `points` for a player in this week, which is the only thing
    /// `apply_gameweek_price_adjustments` ranks on when deciding who moves.
    async fn score(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        week_id: Uuid,
        player_id: Uuid,
        points: i32,
    ) {
        sqlx::query(
            "INSERT INTO player_points (player_id, match_week_id, total_points)
             VALUES ($1, $2, $3)
             ON CONFLICT (player_id, match_week_id) DO UPDATE SET total_points = EXCLUDED.total_points",
        )
        .bind(player_id)
        .bind(week_id)
        .bind(points)
        .execute(&mut **tx)
        .await
        .expect("score player");
    }

    async fn budget(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, team_id: Uuid) -> Decimal {
        sqlx::query_scalar("SELECT budget_limit FROM fantasy_teams WHERE id = $1")
            .bind(team_id)
            .fetch_one(&mut **tx)
            .await
            .expect("read budget")
    }

    async fn squad_value(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, team_id: Uuid) -> Decimal {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(p.price), 0) FROM team_players tp
             JOIN players p ON p.id = tp.player_id WHERE tp.team_id = $1",
        )
        .bind(team_id)
        .fetch_one(&mut **tx)
        .await
        .expect("read squad value")
    }

    /// The bug this module exists for.
    ///
    /// A manager who leaves money aside for next week's transfer used to have it
    /// taken: scoring set the budget to the squad's value, so a $2.05 bank was
    /// spent by the league on their behalf. A gameweek in which none of their
    /// players moved in price has no business touching what they can spend.
    #[tokio::test]
    async fn unspent_money_survives_a_gameweek() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        // $70.00 budget, nine players at $7.55 = $67.95 squad, $2.05 banked.
        let f = seed(&mut tx, 9840, "quiet", money(7000), money(755)).await;
        assert_eq!(squad_value(&mut tx, f.team_id).await, money(6795));

        // Six other players take every price move this week: this manager owns
        // none of them, so nothing they hold changes price.
        for i in 0..6 {
            let outsider = add_player(&mut tx, &format!("Budget Outsider {i}"), money(600)).await;
            score(&mut tx, f.week_id, outsider, 50 - i as i32 * 10).await;
        }
        for id in &f.player_ids {
            score(&mut tx, f.week_id, *id, 20).await;
        }

        apply_gameweek_price_and_budget_changes(&mut tx, f.week_id)
            .await
            .expect("apply");

        assert_eq!(
            squad_value(&mut tx, f.team_id).await,
            money(6795),
            "none of their players moved"
        );
        assert_eq!(
            budget(&mut tx, f.team_id).await,
            money(7000),
            "so their budget must not move either"
        );
        assert_eq!(
            budget(&mut tx, f.team_id).await - squad_value(&mut tx, f.team_id).await,
            money(205),
            "the $2.05 they saved is still theirs"
        );

        tx.rollback().await.expect("rollback");
    }

    /// A manager who has registered but not yet picked a squad has nothing that
    /// can move in price. They were skipped by the old statement's join too, so
    /// this is not a regression — it is the case that must keep working now
    /// that every other team's budget is written by a different rule.
    #[tokio::test]
    async fn a_team_with_no_players_keeps_its_budget() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        // Seed a normal team so the week has price moves to make at all, then
        // strip a second team down to nothing.
        let f = seed(&mut tx, 9844, "empty", money(7000), money(755)).await;
        for (i, id) in f.player_ids.iter().enumerate() {
            score(&mut tx, f.week_id, *id, 100 - i as i32).await;
        }

        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (username, email, password_hash, full_name)
             VALUES ('budget_probe_empty_2', 'budget_probe_empty_2@example.test', 'x', 'Empty Probe')
             RETURNING id",
        )
        .fetch_one(&mut *tx)
        .await
        .expect("insert user");
        let empty_team: Uuid = sqlx::query_scalar(
            "INSERT INTO fantasy_teams (user_id, name, budget_limit)
             VALUES ($1, 'Empty FC', 70.00) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .expect("insert team");

        apply_gameweek_price_and_budget_changes(&mut tx, f.week_id)
            .await
            .expect("apply");

        assert_eq!(
            budget(&mut tx, empty_team).await,
            money(7000),
            "a manager with no squad still has the whole budget to spend"
        );

        tx.rollback().await.expect("rollback");
    }

    /// Price moves still change spending power — that part always worked and has
    /// to keep working. What changes is that the bank rides along instead of
    /// being swallowed.
    #[tokio::test]
    async fn price_moves_shift_the_budget_and_leave_the_bank_alone() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        // $70.00 budget, nine players at $7.55 = $67.95 squad, $2.05 banked.
        let f = seed(&mut tx, 9841, "moves", money(7000), money(755)).await;

        // Their three best are this week's top scorers (+0.3, +0.2, +0.1) and
        // their three worst are the bottom (-0.3, -0.2, -0.1): net zero on the
        // squad, but every one of those moves is theirs.
        for (i, id) in f.player_ids.iter().enumerate() {
            score(&mut tx, f.week_id, *id, 100 - i as i32).await;
        }

        apply_gameweek_price_and_budget_changes(&mut tx, f.week_id)
            .await
            .expect("apply");

        let squad_after = squad_value(&mut tx, f.team_id).await;
        assert_eq!(
            budget(&mut tx, f.team_id).await - squad_after,
            money(205),
            "however the prices moved, the bank is untouched"
        );
        assert_eq!(
            budget(&mut tx, f.team_id).await,
            money(7000) + (squad_after - money(6795)),
            "and the budget moved by exactly what the squad moved"
        );

        tx.rollback().await.expect("rollback");
    }

    /// An admin correcting a gameweek's stats re-runs the whole submission, and
    /// `ops/2026-08-30_repair_gameweek_5.sql` is a standing reason to. That used
    /// to reach forward and empty every manager's bank in the *live* week; it
    /// must now be a no-op for money.
    #[tokio::test]
    async fn rescoring_a_week_leaves_every_budget_where_it_was() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        let f = seed(&mut tx, 9842, "rescore", money(7000), money(755)).await;
        for (i, id) in f.player_ids.iter().enumerate() {
            score(&mut tx, f.week_id, *id, 100 - i as i32).await;
        }

        apply_gameweek_price_and_budget_changes(&mut tx, f.week_id)
            .await
            .expect("first run");
        let after_first = budget(&mut tx, f.team_id).await;

        // The same stats are submitted again, so the same players move the same
        // way. Nothing about the manager's money should notice.
        apply_gameweek_price_and_budget_changes(&mut tx, f.week_id)
            .await
            .expect("second run");

        assert_eq!(
            budget(&mut tx, f.team_id).await,
            after_first,
            "re-scoring a week is neither a payday nor a raid"
        );

        tx.rollback().await.expect("rollback");
    }

    /// The case that decided how this is measured.
    ///
    /// A manager banks a price rise, then transfers that player away. When the
    /// admin later re-scores the week, the rise they already earned must stay
    /// earned — clawing it back is the same confiscation in a different guise.
    #[tokio::test]
    async fn a_transfer_before_a_rescore_does_not_cost_the_manager_their_gain() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        let f = seed(&mut tx, 9843, "transfer", money(7000), money(755)).await;
        for (i, id) in f.player_ids.iter().enumerate() {
            score(&mut tx, f.week_id, *id, 100 - i as i32).await;
        }

        apply_gameweek_price_and_budget_changes(&mut tx, f.week_id)
            .await
            .expect("first run");
        let after_first = budget(&mut tx, f.team_id).await;

        // They sell the player who rose the most, for one whose price is fixed.
        let replacement = add_player(&mut tx, "Budget Replacement", money(785)).await;
        sqlx::query("DELETE FROM team_players WHERE team_id = $1 AND player_id = $2")
            .bind(f.team_id)
            .bind(f.player_ids[0])
            .execute(&mut *tx)
            .await
            .expect("transfer out");
        sqlx::query("INSERT INTO team_players (team_id, player_id) VALUES ($1, $2)")
            .bind(f.team_id)
            .bind(replacement)
            .execute(&mut *tx)
            .await
            .expect("transfer in");

        let bank_before_rescore =
            budget(&mut tx, f.team_id).await - squad_value(&mut tx, f.team_id).await;

        apply_gameweek_price_and_budget_changes(&mut tx, f.week_id)
            .await
            .expect("second run");

        assert_eq!(
            budget(&mut tx, f.team_id).await,
            after_first,
            "the rise they banked while holding the player stays banked"
        );
        assert_eq!(
            budget(&mut tx, f.team_id).await - squad_value(&mut tx, f.team_id).await,
            bank_before_rescore,
            "and the correction does not disturb the bank either"
        );

        tx.rollback().await.expect("rollback");
    }
}
