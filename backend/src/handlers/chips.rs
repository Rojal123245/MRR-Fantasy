use axum::{
    extract::{Extension, Path, State},
    Json,
};
use chrono::Utc;
use chrono_tz::America::New_York;
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{
    ActivateChipRequest, ActiveGameweek, ChipInfo, ChipRow, ChipStatusResponse, FantasyTeam,
};

use super::teams::compute_lock_status;
use crate::services::scoring;

fn chip_can_deactivate(chip: &ChipRow) -> bool {
    let today_et = Utc::now().with_timezone(&New_York).date_naive();
    today_et < chip.start_date
}

async fn build_chip_status(
    pool: &sqlx::PgPool,
    team_id: Uuid,
) -> Result<ChipStatusResponse, AppError> {
    let chips = sqlx::query_as::<_, ChipRow>(
        r#"SELECT tc.chip_type, mw.week_number, mw.start_date
           FROM team_chips tc
           INNER JOIN match_weeks mw ON mw.id = tc.match_week_id
           WHERE tc.team_id = $1"#,
    )
    .bind(team_id)
    .fetch_all(pool)
    .await?;

    let tc_chip = chips.iter().find(|c| c.chip_type == "triple_captain");
    let bb_chip = chips.iter().find(|c| c.chip_type == "bench_boost");

    let active_gw = sqlx::query_as::<_, (Uuid, i32)>(
        "SELECT id, week_number FROM match_weeks WHERE is_active = true LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    Ok(ChipStatusResponse {
        triple_captain: ChipInfo {
            available: tc_chip.is_none(),
            used_in_week: tc_chip.map(|c| c.week_number),
            can_deactivate: tc_chip.map_or(false, chip_can_deactivate),
        },
        bench_boost: ChipInfo {
            available: bb_chip.is_none(),
            used_in_week: bb_chip.map(|c| c.week_number),
            can_deactivate: bb_chip.map_or(false, chip_can_deactivate),
        },
        chip_played_this_week: active_gw.and_then(|(_, week_number)| {
            chips
                .iter()
                .find(|c| c.week_number == week_number)
                .map(|c| c.chip_type.clone())
        }),
        active_gameweek: active_gw.map(|(id, week_number)| ActiveGameweek { id, week_number }),
    })
}

/// GET /api/teams/:id/chips
///
/// Get chip status for a team (which chips are available/used).
pub async fn get_chip_status(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(team_id): Path<Uuid>,
) -> AppResult<Json<ChipStatusResponse>> {
    let _team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    let status = build_chip_status(&state.pool, team_id).await?;
    Ok(Json(status))
}

/// POST /api/teams/:id/chips
///
/// Activate a chip (triple_captain or bench_boost) for the current active gameweek.
/// Each chip can only be used once per team. Can be deactivated before the gameweek starts.
pub async fn activate_chip(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(team_id): Path<Uuid>,
    Json(body): Json<ActivateChipRequest>,
) -> AppResult<Json<ChipStatusResponse>> {
    let lock = compute_lock_status(&state.pool).await?;
    if lock.locked {
        return Err(AppError::BadRequest(
            "Chips close at the end of Saturday. They reopen Sunday 12:00 PM ET.".to_string(),
        ));
    }

    if body.chip_type != "triple_captain" && body.chip_type != "bench_boost" {
        return Err(AppError::BadRequest(
            "Invalid chip type. Must be 'triple_captain' or 'bench_boost'".to_string(),
        ));
    }

    let _team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    let active_gw = sqlx::query_as::<_, (Uuid, i32)>(
        "SELECT id, week_number FROM match_weeks WHERE is_active = true LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest("No active gameweek. Cannot activate chip right now.".to_string())
    })?;


    // A chip is played on a gameweek, so it is worthless once that week's
    // deadline has passed — and it cannot be played again. Gameweek 3 ate a
    // Triple Captain this way, which had to be handed back by an ops script.
    if !scoring::week_accepts_changes(&mut *state.pool.acquire().await?, active_gw.0).await? {
        return Err(AppError::BadRequest(format!(
            "Gameweek {} closed at the end of Saturday. Chips can be played on the \
             next gameweek once it opens.",
            active_gw.1
        )));
    }
    let already_used = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_chips WHERE team_id = $1 AND chip_type = $2",
    )
    .bind(team_id)
    .bind(&body.chip_type)
    .fetch_one(&state.pool)
    .await?;

    if already_used > 0 {
        return Err(AppError::Conflict(format!(
            "You have already used the {} chip. It can only be activated once.",
            body.chip_type.replace('_', " ")
        )));
    }

    // Only one chip per gameweek. The check above is a different question — it
    // asks whether this chip has been spent at all, this season — and nothing
    // asked the per-week one, so a manager could play a Triple Captain and a
    // Bench Boost on the same week. Both scoring engines add their bonuses
    // independently, so the pair paid out together.
    //
    // One manager did exactly this in gameweek 3. The stored chips then broke
    // the league scoreboard with "more than one row returned by a subquery",
    // and the fix was to make the *read* tolerate the pair
    // (`leagues.rs`, `two_chips_in_one_week_do_not_break_the_scoreboard`).
    // The write was never closed, so the illegal state stayed creatable.
    let chip_this_week = sqlx::query_scalar::<_, Option<String>>(
        "SELECT chip_type FROM team_chips WHERE team_id = $1 AND match_week_id = $2 LIMIT 1",
    )
    .bind(team_id)
    .bind(active_gw.0)
    .fetch_optional(&state.pool)
    .await?
    .flatten();

    if let Some(played) = chip_this_week {
        return Err(AppError::Conflict(format!(
            "You have already played the {} chip on gameweek {}. Only one chip \
             can be played per gameweek.",
            played.replace('_', " "),
            active_gw.1
        )));
    }

    sqlx::query("INSERT INTO team_chips (team_id, chip_type, match_week_id) VALUES ($1, $2, $3)")
        .bind(team_id)
        .bind(&body.chip_type)
        .bind(active_gw.0)
        .execute(&state.pool)
        .await?;

    tracing::info!(
        "Chip '{}' activated for team {} in gameweek {}",
        body.chip_type,
        team_id,
        active_gw.1
    );

    let status = build_chip_status(&state.pool, team_id).await?;
    Ok(Json(status))
}

/// DELETE /api/teams/:id/chips/:chip_type
///
/// Deactivate a chip before its gameweek starts. Once the gameweek has begun
/// the chip is locked in and cannot be cancelled.
pub async fn deactivate_chip(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((team_id, chip_type)): Path<(Uuid, String)>,
) -> AppResult<Json<ChipStatusResponse>> {
    if chip_type != "triple_captain" && chip_type != "bench_boost" {
        return Err(AppError::BadRequest(
            "Invalid chip type. Must be 'triple_captain' or 'bench_boost'".to_string(),
        ));
    }

    let _team = sqlx::query_as::<_, FantasyTeam>(
        "SELECT id, user_id, name, captain_id, budget_limit, created_at FROM fantasy_teams WHERE id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team not found or access denied".to_string()))?;

    let chip = sqlx::query_as::<_, ChipRow>(
        r#"SELECT tc.chip_type, mw.week_number, mw.start_date
           FROM team_chips tc
           INNER JOIN match_weeks mw ON mw.id = tc.match_week_id
           WHERE tc.team_id = $1 AND tc.chip_type = $2"#,
    )
    .bind(team_id)
    .bind(&chip_type)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Chip not found or not activated".to_string()))?;

    if !chip_can_deactivate(&chip) {
        return Err(AppError::BadRequest(format!(
            "Cannot deactivate {} — gameweek {} has already started. The chip is permanently used.",
            chip_type.replace('_', " "),
            chip.week_number
        )));
    }

    sqlx::query("DELETE FROM team_chips WHERE team_id = $1 AND chip_type = $2")
        .bind(team_id)
        .bind(&chip_type)
        .execute(&state.pool)
        .await?;

    tracing::info!(
        "Chip '{}' deactivated for team {} (was set for gameweek {})",
        chip_type,
        team_id,
        chip.week_number
    );

    let status = build_chip_status(&state.pool, team_id).await?;
    Ok(Json(status))
}

#[cfg(test)]
mod one_chip_per_week_tests {
    use super::*;
    use crate::test_support::pool;

    /// A manager, a gameweek, and whatever chips they have already played.
    async fn seed(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        week_number: i32,
        tag: &str,
    ) -> (Uuid, Uuid) {
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
             VALUES ($1, $2, 'x', 'Chip Probe') RETURNING id",
        )
        .bind(format!("chip_probe_{tag}"))
        .bind(format!("chip_probe_{tag}@example.test"))
        .fetch_one(&mut **tx)
        .await
        .expect("insert user");

        let team_id: Uuid = sqlx::query_scalar(
            "INSERT INTO fantasy_teams (user_id, name) VALUES ($1, 'Chip FC') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await
        .expect("insert team");

        (week_id, team_id)
    }

    /// The query `activate_chip` uses to decide whether a chip is already
    /// booked for this gameweek. Reproduced here rather than reached through
    /// the handler, which needs an `AppState` and a live router.
    async fn chip_booked_this_week(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        team_id: Uuid,
        week_id: Uuid,
    ) -> Option<String> {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT chip_type FROM team_chips WHERE team_id = $1 AND match_week_id = $2 LIMIT 1",
        )
        .bind(team_id)
        .bind(week_id)
        .fetch_optional(&mut **tx)
        .await
        .expect("read chip")
        .flatten()
    }

    /// PREMIER_LEAGUE_FANTASY_RULES.md: "Only one chip can be played per
    /// gameweek." The season-level guard cannot see this — it asks whether
    /// *this* chip has been spent, and a Bench Boost is not a Triple Captain —
    /// so a manager played both on gameweek 3 and was paid for both.
    #[tokio::test]
    async fn a_second_chip_on_the_same_week_is_refused() {
        let Some(pool) = pool().await else { return };
        let mut tx = pool.begin().await.expect("begin");

        let (week_id, team_id) = seed(&mut tx, 9880, "second").await;

        sqlx::query(
            "INSERT INTO team_chips (team_id, chip_type, match_week_id) VALUES ($1, 'triple_captain', $2)",
        )
        .bind(team_id)
        .bind(week_id)
        .execute(&mut *tx)
        .await
        .expect("play the first chip");

        assert_eq!(
            chip_booked_this_week(&mut tx, team_id, week_id).await.as_deref(),
            Some("triple_captain"),
            "the week already has a chip, so a Bench Boost must be refused"
        );

        tx.rollback().await.expect("rollback");
    }

    /// The same chip on a *different* week is a separate question, and the
    /// season-level guard is what answers it. The per-week check must not
    /// start refusing a manager's first chip of a fresh gameweek.
    #[tokio::test]
    async fn a_chip_on_a_week_that_has_none_is_allowed() {
        let Some(pool) = pool().await else { return };
        let mut tx = pool.begin().await.expect("begin");

        let (played_week, team_id) = seed(&mut tx, 9881, "fresh_a").await;
        let fresh_week: Uuid = sqlx::query_scalar(
            "INSERT INTO match_weeks (week_number, start_date, end_date, is_active)
             VALUES (9882, '2099-01-12'::date, '2099-01-18'::date, false) RETURNING id",
        )
        .fetch_one(&mut *tx)
        .await
        .expect("insert second week");

        sqlx::query(
            "INSERT INTO team_chips (team_id, chip_type, match_week_id) VALUES ($1, 'triple_captain', $2)",
        )
        .bind(team_id)
        .bind(played_week)
        .execute(&mut *tx)
        .await
        .expect("play a chip on the earlier week");

        assert_eq!(
            chip_booked_this_week(&mut tx, team_id, fresh_week).await,
            None,
            "a new gameweek starts with no chip played"
        );

        tx.rollback().await.expect("rollback");
    }
}
