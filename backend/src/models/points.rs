use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
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
