use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Player position enum matching the DB enum.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "player_position", rename_all = "UPPERCASE")]
pub enum PlayerPosition {
    #[serde(rename = "GK")]
    #[sqlx(rename = "GK")]
    Gk,
    #[serde(rename = "DEF")]
    #[sqlx(rename = "DEF")]
    Def,
    #[serde(rename = "MID")]
    #[sqlx(rename = "MID")]
    Mid,
    #[serde(rename = "FWD")]
    #[sqlx(rename = "FWD")]
    Fwd,
}

/// Database row for a football player.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Player {
    pub id: Uuid,
    pub name: String,
    pub position: PlayerPosition,
    pub secondary_position: Option<PlayerPosition>,
    pub is_top_player: bool,
    pub team_name: String,
    pub photo_url: Option<String>,
    pub price: Decimal,
    pub total_points: i32,
    pub created_at: DateTime<Utc>,
}

/// Query parameters for listing players.
#[derive(Debug, Deserialize)]
pub struct PlayerQuery {
    pub position: Option<String>,
    pub search: Option<String>,
}
