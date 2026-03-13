use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::player::PlayerPosition;
use super::Player;

/// Database row for a fantasy team.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FantasyTeam {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub captain_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// A starter player with the position they are assigned to play.
#[derive(Debug, Clone, Serialize)]
pub struct StarterPlayer {
    #[serde(flatten)]
    pub player: Player,
    pub assigned_position: PlayerPosition,
}

/// Fantasy team with its players included.
#[derive(Debug, Serialize)]
pub struct FantasyTeamWithPlayers {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub captain_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub players: Vec<StarterPlayer>,
    pub bench: Vec<Player>,
    pub total_points: i32,
}

/// Request to create a fantasy team.
#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
}

/// A single starter assignment: which player plays in which position.
#[derive(Debug, Deserialize)]
pub struct StarterAssignment {
    pub player_id: Uuid,
    pub assigned_position: PlayerPosition,
}

/// Request to set the 9 players on a team (6 starters with positions + 3 bench).
#[derive(Debug, Deserialize)]
pub struct SetPlayersRequest {
    pub starters: Vec<StarterAssignment>,
    pub bench_player_ids: Vec<Uuid>,
    pub captain_id: Uuid,
}

/// Info about a single chip (Triple Captain or Bench Boost).
#[derive(Debug, Serialize)]
pub struct ChipInfo {
    pub available: bool,
    pub used_in_week: Option<i32>,
}

/// Current active gameweek summary.
#[derive(Debug, Serialize)]
pub struct ActiveGameweek {
    pub id: Uuid,
    pub week_number: i32,
}

/// Combined chip status for a team.
#[derive(Debug, Serialize)]
pub struct ChipStatusResponse {
    pub triple_captain: ChipInfo,
    pub bench_boost: ChipInfo,
    pub active_gameweek: Option<ActiveGameweek>,
}

/// Request to activate a chip.
#[derive(Debug, Deserialize)]
pub struct ActivateChipRequest {
    pub chip_type: String,
}

/// Row returned when querying chip usage.
#[derive(Debug, sqlx::FromRow)]
pub struct ChipRow {
    pub chip_type: String,
    pub week_number: i32,
}
