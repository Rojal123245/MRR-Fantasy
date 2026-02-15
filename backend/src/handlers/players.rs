use axum::{
    extract::{Path, Query, State},
    Json,
};
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::error::{AppError, AppResult};
use crate::models::{Player, PlayerQuery};

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
