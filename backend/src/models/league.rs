use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Database row for a league.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct League {
    pub id: Uuid,
    pub name: String,
    pub invite_code: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// A league member with user info and points.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LeagueMemberStanding {
    pub user_id: Uuid,
    pub username: String,
    pub team_name: Option<String>,
    pub total_points: Option<i64>,
}

/// League detail with members.
#[derive(Debug, Serialize)]
pub struct LeagueDetail {
    pub league: League,
    pub members: Vec<LeagueMemberStanding>,
}

/// Request to create a league.
#[derive(Debug, Deserialize)]
pub struct CreateLeagueRequest {
    pub name: String,
}

/// Request to join a league.
#[derive(Debug, Deserialize)]
pub struct JoinLeagueRequest {
    pub invite_code: String,
}
