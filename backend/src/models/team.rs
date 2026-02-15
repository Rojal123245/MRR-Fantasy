use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Player;

/// Database row for a fantasy team.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FantasyTeam {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Fantasy team with its players included.
#[derive(Debug, Serialize)]
pub struct FantasyTeamWithPlayers {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub players: Vec<Player>,
    pub bench: Vec<Player>,
    pub total_points: i32,
}

/// Request to create a fantasy team.
#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
}

/// Request to set the 9 players on a team (6 starters + 3 bench).
#[derive(Debug, Deserialize)]
pub struct SetPlayersRequest {
    pub player_ids: Vec<Uuid>,
    pub bench_player_ids: Vec<Uuid>,
}
