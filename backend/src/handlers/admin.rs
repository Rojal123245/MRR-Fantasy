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
    // Taken here, *after* the `fantasy_teams` update above, so every
    // transaction that touches both takes `fantasy_teams` first and no cycle
    // can form with a save holding a team's row and waiting on the week.
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
mod price_adjustment_tests {
    use super::*;

    async fn pool() -> Option<sqlx::PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        sqlx::PgPool::connect(&url).await.ok()
    }

    fn money(cents: i64) -> Decimal {
        Decimal::new(cents, 2)
    }

    /// Six disposable players and three gameweeks, inside a transaction the test
    /// rolls back.
    ///
    /// Six is what it takes to own every slot a week hands out: the fixture's
    /// players are the only ones with points in these weeks, so the top three
    /// and the bottom three are all its own and no real player's price is
    /// touched. Bottom is reached by scoring *below* zero, since every other
    /// player in the database sits at zero for a week that never happened.
    struct Fixture<'a> {
        tx: sqlx::Transaction<'a, sqlx::Postgres>,
        players: Vec<Uuid>,
        weeks: Vec<Uuid>,
    }

    /// Fixture weeks sit far in the future and far above every real week number,
    /// so `week_number >= N` can never reach back into real history.
    const BASE_WEEK: i32 = 9860;

    impl<'a> Fixture<'a> {
        /// `tag` keeps player names — and so the ordering tiebreak — unique per
        /// test; `base_week` must be too, since `match_weeks.week_number` is
        /// unique and two uncommitted transactions inserting the same number
        /// would block.
        async fn open(
            pool: &'a sqlx::PgPool,
            tag: &str,
            base_week: i32,
            prices: [i64; 6],
        ) -> Fixture<'a> {
            let mut tx = pool.begin().await.expect("begin");

            let mut players = Vec::new();
            for (i, cents) in prices.iter().enumerate() {
                let id: Uuid = sqlx::query_scalar(
                    "INSERT INTO players (name, position, team_name, price)
                     VALUES ($1, 'MID', 'Price Fixture FC', $2) RETURNING id",
                )
                .bind(format!("price_{tag}_{i}"))
                .bind(money(*cents))
                .fetch_one(&mut *tx)
                .await
                .expect("insert player");
                players.push(id);
            }

            let mut weeks = Vec::new();
            for offset in 0..3 {
                let start = chrono::NaiveDate::from_ymd_opt(2099, 1, 5)
                    .expect("valid epoch")
                    + chrono::Duration::days(7 * offset as i64);
                let id: Uuid = sqlx::query_scalar(
                    "INSERT INTO match_weeks (week_number, start_date, end_date, is_active)
                     VALUES ($1, $2, $3, false) RETURNING id",
                )
                .bind(base_week + offset)
                .bind(start)
                .bind(start + chrono::Duration::days(6))
                .fetch_one(&mut *tx)
                .await
                .expect("insert match week");
                weeks.push(id);
            }

            Fixture { tx, players, weeks }
        }

        /// Score the fixture's six players for one week, in `players` order.
        async fn score(&mut self, week: usize, points: [i32; 6]) {
            for (i, pts) in points.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO player_points (player_id, match_week_id, total_points)
                     VALUES ($1, $2, $3)",
                )
                .bind(self.players[i])
                .bind(self.weeks[week])
                .bind(pts)
                .execute(&mut *self.tx)
                .await
                .expect("insert player points");
            }
        }

        async fn price(&mut self, player: usize) -> Decimal {
            sqlx::query_scalar("SELECT price FROM players WHERE id = $1")
                .bind(self.players[player])
                .fetch_one(&mut *self.tx)
                .await
                .expect("select price")
        }

        /// Everything the ledger says has ever been done to this player's price.
        async fn recorded_moves(&mut self, player: usize) -> Decimal {
            sqlx::query_scalar(
                "SELECT COALESCE(SUM(delta), 0) FROM gameweek_price_adjustments \
                 WHERE player_id = $1",
            )
            .bind(self.players[player])
            .fetch_one(&mut *self.tx)
            .await
            .expect("sum deltas")
        }

        async fn set_price(&mut self, player: usize, cents: i64) {
            sqlx::query("UPDATE players SET price = $1 WHERE id = $2")
                .bind(money(cents))
                .bind(self.players[player])
                .execute(&mut *self.tx)
                .await
                .expect("set price");
        }

        async fn adjust(&mut self, week: usize) -> Result<(), AppError> {
            apply_gameweek_price_adjustments(&mut self.tx, self.weeks[week]).await
        }

        async fn close(self) {
            self.tx.rollback().await.expect("rollback");
        }
    }

    /// Player 0 tops week 0, then props up the bottom of weeks 1 and 2. The
    /// others fill the slots around them.
    const TOP_THEN_BOTTOM: [i32; 6] = [10, 8, 6, -3, -5, -10];
    const BOTTOM: [i32; 6] = [-10, 10, 8, 6, -3, -5];

    /// A price must never drift above the moves recorded against it.
    ///
    /// Week 0 lifts a $0.40 player to $0.70; weeks 1 and 2 take $0.30 each and
    /// leave them on the $0.10 floor. Re-scoring week 0 with the same stats — the
    /// documented repair workflow, `ops/2026-08-30_repair_gameweek_5.sql` step 4 —
    /// has to leave the same $0.10 behind.
    ///
    /// It used to leave $0.40. Undoing week 0's rise from a price that no longer
    /// had room for it was clamped at the floor, the rise was re-applied on top
    /// regardless, and the $0.30 the clamp swallowed became budget for every
    /// squad holding them.
    #[tokio::test]
    async fn re_scoring_a_week_leaves_the_price_its_ledger_implies() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };

        let mut f = Fixture::open(&pool, "rescore", BASE_WEEK, [40, 500, 500, 500, 500, 500]).await;
        f.score(0, TOP_THEN_BOTTOM).await;
        f.score(1, BOTTOM).await;
        f.score(2, BOTTOM).await;

        for week in 0..3 {
            f.adjust(week).await.expect("scoring a week adjusts prices");
        }
        assert_eq!(
            f.price(0).await,
            money(10),
            "0.40 +0.30 -0.30 -0.30 should sit exactly on the floor",
        );

        f.adjust(0).await.expect("re-scoring week 0 should succeed");

        assert_eq!(
            f.price(0).await,
            money(10),
            "re-scoring week 0 with unchanged stats moved the price",
        );

        // The invariant behind that number, held for every player the weeks
        // touched: a price is its opening price plus everything the ledger
        // recorded, and nothing else.
        for (player, opening) in [(0usize, 40i64), (1, 500), (2, 500), (3, 500), (4, 500), (5, 500)]
        {
            let moves = f.recorded_moves(player).await;
            assert_eq!(
                f.price(player).await,
                money(opening) + moves,
                "player {player} is not their opening price plus their recorded moves ({moves})",
            );
        }

        f.close().await;
    }

    /// A price edited outside the ledger cannot be rebuilt from it, and saying so
    /// is the point: the alternative is absorbing the difference silently, which
    /// is what made a player dearer than their recorded moves in the first place.
    #[tokio::test]
    async fn re_scoring_refuses_when_a_price_moved_outside_the_ledger() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };

        let mut f =
            Fixture::open(&pool, "offledger", BASE_WEEK + 10, [40, 500, 500, 500, 500, 500]).await;
        f.score(0, TOP_THEN_BOTTOM).await;
        f.adjust(0).await.expect("scoring week 0 adjusts prices");

        // By hand, in the database: a $0.30 rise now has $0.00 of room to give back.
        f.set_price(0, 10).await;

        let err = f
            .adjust(0)
            .await
            .expect_err("re-scoring should refuse a price it cannot undo");
        assert!(
            matches!(err, AppError::Conflict(ref m) if m.contains("below the")),
            "expected a conflict naming the floor, got {err:?}",
        );

        f.close().await;
    }
}
