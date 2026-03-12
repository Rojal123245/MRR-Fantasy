use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Database row for a match week.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MatchWeek {
    pub id: Uuid,
    pub week_number: i32,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
}

/// Database row for player points in a match week.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PlayerPoints {
    pub id: Uuid,
    pub player_id: Uuid,
    pub match_week_id: Uuid,
    pub goals: i32,
    pub assists: i32,
    pub clean_sheets: i32,
    pub saves: i32,
    pub tackles: i32,
    pub total_points: i32,
    pub own_goals: i32,
    pub penalty_misses: i32,
    pub penalty_saves: i32,
    pub regular_fouls: i32,
    pub serious_fouls: i32,
    pub minutes_played: i32,
    pub created_at: DateTime<Utc>,
}

/// Player points with player name for display.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PlayerPointsDisplay {
    pub player_id: Uuid,
    pub player_name: String,
    pub position: String,
    pub goals: i32,
    pub assists: i32,
    pub clean_sheets: i32,
    pub saves: i32,
    pub tackles: i32,
    pub total_points: i32,
    pub week_number: i32,
}

/// Admin view of player stats for a gameweek.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminPlayerStats {
    pub player_id: Uuid,
    pub player_name: String,
    pub position: String,
    pub goals: i32,
    pub assists: i32,
    pub clean_sheets: i32,
    pub saves: i32,
    pub penalty_saves: i32,
    pub own_goals: i32,
    pub penalty_misses: i32,
    pub regular_fouls: i32,
    pub serious_fouls: i32,
    pub minutes_played: i32,
    pub total_points: i32,
}

/// Request body for submitting a single player's stats.
#[derive(Debug, Deserialize)]
pub struct PlayerStatInput {
    pub player_id: Uuid,
    pub goals: i32,
    pub assists: i32,
    pub clean_sheets: i32,
    pub saves: i32,
    pub penalty_saves: i32,
    pub own_goals: i32,
    pub penalty_misses: i32,
    pub regular_fouls: i32,
    pub serious_fouls: i32,
    pub minutes_played: i32,
}

/// Request body for creating a new gameweek.
#[derive(Debug, Deserialize)]
pub struct CreateGameweekRequest {
    pub week_number: i32,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}
