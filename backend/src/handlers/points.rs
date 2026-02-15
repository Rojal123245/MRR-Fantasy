use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::error::AppResult;
use crate::models::PlayerPointsDisplay;

/// GET /api/points/week/:week
///
/// Get all player points for a specific match week.
pub async fn get_week_points(
    State(state): State<AppState>,
    Path(week_number): Path<i32>,
) -> AppResult<Json<Vec<PlayerPointsDisplay>>> {
    let points = sqlx::query_as::<_, PlayerPointsDisplay>(
        r#"SELECT
             pp.player_id,
             p.name AS player_name,
             p.position::text AS position,
             pp.goals,
             pp.assists,
             pp.clean_sheets,
             pp.saves,
             pp.tackles,
             pp.total_points,
             mw.week_number
           FROM player_points pp
           INNER JOIN players p ON p.id = pp.player_id
           INNER JOIN match_weeks mw ON mw.id = pp.match_week_id
           WHERE mw.week_number = $1
           ORDER BY pp.total_points DESC"#,
    )
    .bind(week_number)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(points))
}

/// GET /api/points/player/:id
///
/// Get a player's point history across all match weeks.
pub async fn get_player_points(
    State(state): State<AppState>,
    Path(player_id): Path<Uuid>,
) -> AppResult<Json<Vec<PlayerPointsDisplay>>> {
    let points = sqlx::query_as::<_, PlayerPointsDisplay>(
        r#"SELECT
             pp.player_id,
             p.name AS player_name,
             p.position::text AS position,
             pp.goals,
             pp.assists,
             pp.clean_sheets,
             pp.saves,
             pp.tackles,
             pp.total_points,
             mw.week_number
           FROM player_points pp
           INNER JOIN players p ON p.id = pp.player_id
           INNER JOIN match_weeks mw ON mw.id = pp.match_week_id
           WHERE pp.player_id = $1
           ORDER BY mw.week_number ASC"#,
    )
    .bind(player_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(points))
}
