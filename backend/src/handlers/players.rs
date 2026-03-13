use axum::{
    extract::{Path, Query, State},
    Json,
};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::error::{AppError, AppResult};
use crate::models::{Player, PlayerLeaderboard, PlayerQuery};

/// GET /api/players
///
/// List all available players with optional position and search filters.
pub async fn list_players(
    State(state): State<AppState>,
    Query(query): Query<PlayerQuery>,
) -> AppResult<Json<Vec<Player>>> {
    let players = match (&query.position, &query.search) {
        (Some(pos), Some(search)) => {
            let search_pattern = format!("%{search}%");
            sqlx::query_as::<_, Player>(
                r#"SELECT id, name, position, secondary_position, is_top_player, team_name, photo_url, price, total_points, created_at
                   FROM players
                   WHERE (position::text = $1 OR secondary_position::text = $1) AND name ILIKE $2
                   ORDER BY total_points DESC"#,
            )
            .bind(pos)
            .bind(&search_pattern)
            .fetch_all(&state.pool)
            .await?
        }
        (Some(pos), None) => {
            sqlx::query_as::<_, Player>(
                r#"SELECT id, name, position, secondary_position, is_top_player, team_name, photo_url, price, total_points, created_at
                   FROM players
                   WHERE position::text = $1 OR secondary_position::text = $1
                   ORDER BY total_points DESC"#,
            )
            .bind(pos)
            .fetch_all(&state.pool)
            .await?
        }
        (None, Some(search)) => {
            let search_pattern = format!("%{search}%");
            sqlx::query_as::<_, Player>(
                r#"SELECT id, name, position, secondary_position, is_top_player, team_name, photo_url, price, total_points, created_at
                   FROM players
                   WHERE name ILIKE $1
                   ORDER BY total_points DESC"#,
            )
            .bind(&search_pattern)
            .fetch_all(&state.pool)
            .await?
        }
        (None, None) => {
            sqlx::query_as::<_, Player>(
                r#"SELECT id, name, position, secondary_position, is_top_player, team_name, photo_url, price, total_points, created_at
                   FROM players
                   ORDER BY total_points DESC"#,
            )
            .fetch_all(&state.pool)
            .await?
        }
    };

    Ok(Json(players))
}

/// GET /api/players/leaderboard
///
/// Player leaderboard with aggregated stats per position and chosen-by percentage.
pub async fn leaderboard(
    State(state): State<AppState>,
    Query(query): Query<PlayerQuery>,
) -> AppResult<Json<Vec<PlayerLeaderboard>>> {
    let rows = match &query.position {
        Some(pos) => {
            sqlx::query_as::<_, PlayerLeaderboard>(
                r#"SELECT
                     p.id,
                     p.name,
                     p.position,
                     p.team_name,
                     p.photo_url,
                     p.is_top_player,
                     COALESCE(SUM(pp.goals), 0) AS goals,
                     COALESCE(SUM(pp.assists), 0) AS assists,
                     COALESCE(SUM(pp.clean_sheets), 0) AS clean_sheets,
                     COALESCE(SUM(pp.saves), 0) AS saves,
                     p.total_points,
                     COALESCE(
                       (SELECT COUNT(DISTINCT tp.team_id) FROM team_players tp WHERE tp.player_id = p.id)::float
                       / GREATEST((SELECT COUNT(*) FROM fantasy_teams), 1)::float * 100.0,
                       0.0
                     ) AS chosen_by_percent
                   FROM players p
                   LEFT JOIN player_points pp ON pp.player_id = p.id
                   WHERE p.position::text = $1 OR p.secondary_position::text = $1
                   GROUP BY p.id, p.name, p.position, p.team_name, p.photo_url, p.is_top_player, p.total_points
                   ORDER BY chosen_by_percent DESC, p.total_points DESC"#,
            )
            .bind(pos)
            .fetch_all(&state.pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, PlayerLeaderboard>(
                r#"SELECT
                     p.id,
                     p.name,
                     p.position,
                     p.team_name,
                     p.photo_url,
                     p.is_top_player,
                     COALESCE(SUM(pp.goals), 0) AS goals,
                     COALESCE(SUM(pp.assists), 0) AS assists,
                     COALESCE(SUM(pp.clean_sheets), 0) AS clean_sheets,
                     COALESCE(SUM(pp.saves), 0) AS saves,
                     p.total_points,
                     COALESCE(
                       (SELECT COUNT(DISTINCT tp.team_id) FROM team_players tp WHERE tp.player_id = p.id)::float
                       / GREATEST((SELECT COUNT(*) FROM fantasy_teams), 1)::float * 100.0,
                       0.0
                     ) AS chosen_by_percent
                   FROM players p
                   LEFT JOIN player_points pp ON pp.player_id = p.id
                   GROUP BY p.id, p.name, p.position, p.team_name, p.photo_url, p.is_top_player, p.total_points
                   ORDER BY chosen_by_percent DESC, p.total_points DESC"#,
            )
            .fetch_all(&state.pool)
            .await?
        }
    };

    Ok(Json(rows))
}

/// GET /api/players/:id
///
/// Get a single player's details.
pub async fn get_player(
    State(state): State<AppState>,
    Path(player_id): Path<Uuid>,
) -> AppResult<Json<Player>> {
    let player = sqlx::query_as::<_, Player>(
        r#"SELECT id, name, position, secondary_position, is_top_player, team_name, photo_url, price, total_points, created_at
           FROM players WHERE id = $1"#,
    )
    .bind(player_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Player not found".to_string()))?;

    Ok(Json(player))
}
