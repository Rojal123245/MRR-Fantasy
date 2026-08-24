use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{PlayerPosition, StarterPlayer};

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
    pub full_name: String,
    pub team_name: Option<String>,
    pub total_points: Option<i64>,
}

/// Response for viewing a league member's starting lineup.
#[derive(Debug, Serialize)]
pub struct MemberLineupResponse {
    pub user_id: Uuid,
    pub username: String,
    pub team_name: String,
    pub captain_id: Option<Uuid>,
    pub starters: Vec<StarterPlayer>,
}

/// League detail with members.
#[derive(Debug, Serialize)]
pub struct LeagueDetail {
    pub league: League,
    pub members: Vec<LeagueMemberStanding>,
}

/// Summary of a league the user belongs to.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MyLeague {
    pub id: Uuid,
    pub name: String,
    pub invite_code: String,
    pub member_count: i64,
    pub created_at: DateTime<Utc>,
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

/// A league member's points for a single gameweek.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LeagueGameweekStanding {
    pub user_id: Uuid,
    pub username: String,
    pub full_name: String,
    pub team_name: Option<String>,
    pub week_number: i32,
    pub gameweek_points: Option<i64>,
}

/// Response for per-gameweek league standings.
#[derive(Debug, Serialize)]
pub struct LeagueGameweekDetail {
    pub league_id: Uuid,
    pub week_number: i32,
    pub members: Vec<LeagueGameweekStanding>,
}

/// One squad member's line in a completed gameweek, as read from that week's
/// lineup snapshot.
///
/// The component fields are the arithmetic behind `base_points`: they add up to
/// it exactly, so a manager can check the total by eye. `counted` is false for a
/// bench player in a week without Bench Boost — their line is still shown, worth
/// what it was worth, but it did not reach the team total.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GameweekPlayerLine {
    pub id: Uuid,
    pub name: String,
    pub position: PlayerPosition,
    pub secondary_position: Option<PlayerPosition>,
    pub team_name: String,
    pub photo_url: Option<String>,
    /// The role the manager played them in that week, which sets their rates.
    pub played_as: String,
    pub is_bench: bool,
    pub is_captain: bool,

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

    pub goal_points: i32,
    pub assist_points: i32,
    pub clean_sheet_points: i32,
    pub save_points: i32,
    pub minutes_points: i32,
    pub deduction_points: i32,

    /// The six components above, summed.
    pub base_points: i32,
    /// 1, 2 as captain, or 3 under Triple Captain.
    pub multiplier: i32,
    /// Whether this line reached the team total.
    pub counted: bool,
    /// `base_points * multiplier`, or 0 when the line did not count.
    pub total_points: i32,
}

/// A manager's completed gameweek: what they scored and how.
#[derive(Debug, Serialize)]
pub struct MemberGameweekResponse {
    pub user_id: Uuid,
    pub username: String,
    pub team_name: String,
    pub week_number: i32,
    /// False when this gameweek predates lineup snapshots for this team. The
    /// lineup is then genuinely unknown, and the arrays below are empty — the
    /// live squad is never substituted, because scoring a settled week from
    /// today's squad is what made past weeks mutate.
    pub has_snapshot: bool,
    pub captain_id: Option<Uuid>,
    /// Every chip this team played that week. Usually empty or one, but the
    /// schema allows a Triple Captain and a Bench Boost in the same week and a
    /// manager has done exactly that, so this is a list.
    pub chips_played: Vec<String>,
    /// Stored totals, present once the week has been scored.
    pub gross_points: Option<i32>,
    pub transfer_points_hit: Option<i32>,
    pub total_points: Option<i32>,
    pub starters: Vec<GameweekPlayerLine>,
    pub bench: Vec<GameweekPlayerLine>,
}

/// One row of a gameweek scoreboard.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GameweekScoreboardEntry {
    pub user_id: Uuid,
    pub username: String,
    pub full_name: String,
    pub team_name: Option<String>,
    pub gross_points: Option<i32>,
    pub transfer_points_hit: Option<i32>,
    pub total_points: Option<i32>,
    pub chips_played: Vec<String>,
    /// Whether this manager's lineup for the week can be opened at all.
    pub has_snapshot: bool,
}

/// Who scored what in one gameweek.
#[derive(Debug, Serialize)]
pub struct GameweekScoreboard {
    pub league_id: Uuid,
    pub week_number: i32,
    /// True once the week has been scored, which is also what opens every
    /// manager's lineup to the rest of the league.
    pub is_complete: bool,
    pub entries: Vec<GameweekScoreboardEntry>,
}
