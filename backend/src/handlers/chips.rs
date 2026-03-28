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
            "Chips cannot be activated during the lock period (Saturday 10:00 PM ET to Sunday 12:00 PM ET)".to_string(),
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
