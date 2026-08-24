use axum::{
    extract::{Extension, Path, State},
    response::IntoResponse,
    Json,
};
use rand::Rng;
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};
use crate::handlers::teams::{compute_lock_status, fetch_team_starters};
use crate::models::{
    CreateLeagueRequest, GameweekPlayerLine, GameweekScoreboard, GameweekScoreboardEntry,
    JoinLeagueRequest, League, LeagueDetail, LeagueGameweekDetail, LeagueGameweekStanding,
    LeagueMemberStanding, MemberGameweekResponse, MemberLineupResponse, MyLeague,
};
use crate::services::{points_sql, scoring};

/// Generate a random 8-character alphanumeric invite code.
fn generate_invite_code() -> String {
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();
    (0..8)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}

/// POST /api/leagues
///
/// Create a new league and automatically add the creator as a member.
pub async fn create_league(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateLeagueRequest>,
) -> AppResult<Json<League>> {
    if body.name.is_empty() {
        return Err(AppError::BadRequest(
            "League name cannot be empty".to_string(),
        ));
    }

    let invite_code = generate_invite_code();

    let mut tx = state.pool.begin().await?;

    let league = sqlx::query_as::<_, League>(
        r#"INSERT INTO leagues (name, invite_code, created_by)
           VALUES ($1, $2, $3)
           RETURNING id, name, invite_code, created_by, created_at"#,
    )
    .bind(&body.name)
    .bind(&invite_code)
    .bind(auth.user_id)
    .fetch_one(&mut *tx)
    .await?;

    // Auto-add creator as a member
    sqlx::query("INSERT INTO league_members (league_id, user_id) VALUES ($1, $2)")
        .bind(league.id)
        .bind(auth.user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(league))
}

/// POST /api/leagues/join
///
/// Join a league using an invite code.
pub async fn join_league(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<JoinLeagueRequest>,
) -> AppResult<Json<League>> {
    let league = sqlx::query_as::<_, League>(
        "SELECT id, name, invite_code, created_by, created_at FROM leagues WHERE invite_code = $1",
    )
    .bind(&body.invite_code)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Invalid invite code".to_string()))?;

    // Check if already a member
    let already_member = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM league_members WHERE league_id = $1 AND user_id = $2",
    )
    .bind(league.id)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    if already_member > 0 {
        return Err(AppError::Conflict(
            "You are already a member of this league".to_string(),
        ));
    }

    sqlx::query("INSERT INTO league_members (league_id, user_id) VALUES ($1, $2)")
        .bind(league.id)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await?;

    Ok(Json(league))
}

/// GET /api/leagues/my
///
/// List all leagues the authenticated user belongs to.
pub async fn get_my_leagues(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<Json<Vec<MyLeague>>> {
    let leagues = sqlx::query_as::<_, MyLeague>(
        r#"SELECT
             l.id,
             l.name,
             l.invite_code,
             (SELECT COUNT(*) FROM league_members lm2 WHERE lm2.league_id = l.id) AS member_count,
             l.created_at
           FROM league_members lm
           INNER JOIN leagues l ON l.id = lm.league_id
           WHERE lm.user_id = $1
           ORDER BY l.created_at DESC"#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(leagues))
}

/// GET /api/leagues/:id
///
/// Get league details including member standings.
pub async fn get_league(
    State(state): State<AppState>,
    Path(league_id): Path<Uuid>,
) -> AppResult<Json<LeagueDetail>> {
    let league = sqlx::query_as::<_, League>(
        "SELECT id, name, invite_code, created_by, created_at FROM leagues WHERE id = $1",
    )
    .bind(league_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("League not found".to_string()))?;

    let members = sqlx::query_as::<_, LeagueMemberStanding>(
        r#"SELECT
             u.id AS user_id,
             u.username,
             u.full_name,
             ft.name AS team_name,
             COALESCE((
               SELECT SUM(tgp.total_points)
               FROM team_gameweek_points tgp
               WHERE tgp.team_id = ft.id
             ), 0) AS total_points
           FROM league_members lm
           INNER JOIN users u ON u.id = lm.user_id
           LEFT JOIN fantasy_teams ft ON ft.user_id = u.id
           WHERE lm.league_id = $1
           ORDER BY total_points DESC"#,
    )
    .bind(league_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(LeagueDetail { league, members }))
}

/// GET /api/leagues/:id/leaderboard
///
/// Get ranked leaderboard for a league.
pub async fn get_leaderboard(
    State(state): State<AppState>,
    Path(league_id): Path<Uuid>,
) -> AppResult<Json<Vec<LeagueMemberStanding>>> {
    let standings = sqlx::query_as::<_, LeagueMemberStanding>(
        r#"SELECT
             u.id AS user_id,
             u.username,
             u.full_name,
             ft.name AS team_name,
             COALESCE((
               SELECT SUM(tgp.total_points)
               FROM team_gameweek_points tgp
               WHERE tgp.team_id = ft.id
             ), 0) AS total_points
           FROM league_members lm
           INNER JOIN users u ON u.id = lm.user_id
           LEFT JOIN fantasy_teams ft ON ft.user_id = u.id
           WHERE lm.league_id = $1
           ORDER BY total_points DESC"#,
    )
    .bind(league_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(standings))
}

/// GET /api/leagues/:id/gameweek/:week
///
/// Get league member standings for a specific gameweek.
pub async fn get_league_gameweek(
    State(state): State<AppState>,
    Path((league_id, week)): Path<(Uuid, i32)>,
) -> AppResult<Json<LeagueGameweekDetail>> {
    let _league = sqlx::query_scalar::<_, Uuid>("SELECT id FROM leagues WHERE id = $1")
        .bind(league_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("League not found".to_string()))?;

    let members = sqlx::query_as::<_, LeagueGameweekStanding>(
        r#"SELECT
             u.id AS user_id,
             u.username,
             u.full_name,
             ft.name AS team_name,
             $2 AS week_number,
             COALESCE((
               SELECT tgp.total_points::bigint
               FROM team_gameweek_points tgp
               INNER JOIN match_weeks mw ON mw.id = tgp.match_week_id
               WHERE tgp.team_id = ft.id AND mw.week_number = $2
             ), 0) AS gameweek_points
           FROM league_members lm
           INNER JOIN users u ON u.id = lm.user_id
           LEFT JOIN fantasy_teams ft ON ft.user_id = u.id
           WHERE lm.league_id = $1
           ORDER BY gameweek_points DESC"#,
    )
    .bind(league_id)
    .bind(week)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(LeagueGameweekDetail {
        league_id,
        week_number: week,
        members,
    }))
}

/// GET /api/leagues/:league_id/members/:user_id/lineup
///
/// View a league member's starting 6 lineup. Only available when
/// the lineup is locked (gameweek in progress) and only to fellow
/// league members.
pub async fn get_member_lineup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((league_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<MemberLineupResponse>> {
    let lock = compute_lock_status(&state.pool).await?;
    if !lock.locked {
        return Err(AppError::BadRequest(
            "Lineups become visible once team selection closes at the end of Saturday"
                .to_string(),
        ));
    }

    let requesting_is_member = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM league_members WHERE league_id = $1 AND user_id = $2",
    )
    .bind(league_id)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    if requesting_is_member == 0 {
        return Err(AppError::BadRequest(
            "You are not a member of this league".to_string(),
        ));
    }

    let target_is_member = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM league_members WHERE league_id = $1 AND user_id = $2",
    )
    .bind(league_id)
    .bind(target_user_id)
    .fetch_one(&state.pool)
    .await?;

    if target_is_member == 0 {
        return Err(AppError::NotFound(
            "User is not a member of this league".to_string(),
        ));
    }

    #[derive(sqlx::FromRow)]
    struct TeamRow {
        id: Uuid,
        name: String,
        captain_id: Option<Uuid>,
    }

    let team = sqlx::query_as::<_, TeamRow>(
        "SELECT id, name, captain_id FROM fantasy_teams WHERE user_id = $1",
    )
    .bind(target_user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("This player hasn't created a team yet".to_string()))?;

    let starters = fetch_team_starters(&state.pool, team.id).await?;

    let username = sqlx::query_scalar::<_, String>("SELECT username FROM users WHERE id = $1")
        .bind(target_user_id)
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(MemberLineupResponse {
        user_id: target_user_id,
        username,
        team_name: team.name,
        captain_id: team.captain_id,
        starters,
    }))
}

// ---------------------------------------------------------------------------
// Completed-gameweek lineups and scoreboard
// ---------------------------------------------------------------------------

/// Whether a gameweek's lineups may be read by someone other than their owner.
///
/// This is the whole privacy rule, in one place, decided from database state
/// rather than from anything the caller sends. Every endpoint that can reveal a
/// rival's players goes through [`clear_to_read`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineupAccess {
    /// The week has been scored. It is settled, so it is open to the league.
    Completed,
    /// The live week, with the lineup lock in effect. Open, as it always was.
    LockedInProgress,
    /// The live week before the lock. This is the one that must stay hidden:
    /// seeing a rival's picks now would let a manager copy or counter them.
    OpenForSelection,
    /// A week nobody has played yet, or one that was never scored.
    NotPlayed,
}

/// Resolve a gameweek and decide who may see its lineups.
///
/// `locked` is passed in rather than read here so the rule can be exercised
/// against a real database without depending on what day of the week it is.
async fn week_access(
    conn: &mut sqlx::PgConnection,
    week_number: i32,
    locked: bool,
) -> AppResult<(Uuid, LineupAccess)> {
    #[derive(sqlx::FromRow)]
    struct WeekRow {
        id: Uuid,
        is_active: bool,
    }

    let week = sqlx::query_as::<_, WeekRow>(
        "SELECT id, is_active FROM match_weeks WHERE week_number = $1",
    )
    .bind(week_number)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Gameweek {week_number} not found")))?;

    let access = if scoring::week_already_scored(&mut *conn, week.id).await? {
        // Scored means settled: nothing a rival learns now can change it.
        LineupAccess::Completed
    } else if week.is_active {
        if locked {
            LineupAccess::LockedInProgress
        } else {
            LineupAccess::OpenForSelection
        }
    } else {
        LineupAccess::NotPlayed
    };

    Ok((week.id, access))
}

/// Apply the privacy rule. A manager may always read their own lineup.
fn clear_to_read(
    week_id: Uuid,
    access: LineupAccess,
    viewing_own: bool,
) -> AppResult<ReadableWeek> {
    if viewing_own {
        return Ok(ReadableWeek(week_id));
    }

    match access {
        LineupAccess::Completed | LineupAccess::LockedInProgress => Ok(ReadableWeek(week_id)),
        LineupAccess::OpenForSelection => Err(AppError::Forbidden(
            "Lineups for the current gameweek stay hidden until the deadline. \
             You can see everyone's once the gameweek locks."
                .to_string(),
        )),
        LineupAccess::NotPlayed => Err(AppError::Forbidden(
            "That gameweek has not been played yet.".to_string(),
        )),
    }
}

/// Look up the current lock, then decide. Used by the handlers.
async fn week_state(pool: &sqlx::PgPool, week_number: i32) -> AppResult<(Uuid, LineupAccess)> {
    let locked = compute_lock_status(pool).await?.locked;
    let mut conn = pool.acquire().await?;
    week_access(&mut conn, week_number, locked).await
}

/// A gameweek whose lineups the caller has been cleared to read.
///
/// The field is private and [`clear_to_read`] is the only thing that builds one,
/// so a handler cannot reach [`fetch_gameweek_lines`] without having passed the
/// privacy check first. The rule is enforced by the type, not by remembering to
/// call something.
pub struct ReadableWeek(Uuid);

/// Resolve a gameweek and clear the caller to read its lineups, or refuse.
async fn readable_week(
    pool: &sqlx::PgPool,
    week_number: i32,
    viewing_own: bool,
) -> AppResult<ReadableWeek> {
    let (week_id, access) = week_state(pool, week_number).await?;
    clear_to_read(week_id, access, viewing_own)
}

/// One team's players for a gameweek, from that week's snapshot and nothing else.
///
/// Returns an empty vector when the team has no snapshot for the week, which is
/// the truthful answer for gameweeks that predate snapshots. The live squad is
/// never substituted: scoring a settled week from today's squad is exactly what
/// made past weeks mutate.
async fn fetch_gameweek_lines(
    pool: &sqlx::PgPool,
    team_id: Uuid,
    week: &ReadableWeek,
) -> AppResult<Vec<GameweekPlayerLine>> {
    Ok(
        sqlx::query_as::<_, GameweekPlayerLine>(&points_sql::gameweek_breakdown())
            .bind(team_id)
            .bind(week.0)
            .fetch_all(pool)
            .await?,
    )
}

/// Confirm the caller belongs to the league, and the person they are asking about.
async fn require_both_in_league(
    pool: &sqlx::PgPool,
    league_id: Uuid,
    requester: Uuid,
    target: Uuid,
) -> AppResult<()> {
    let requester_is_member = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM league_members WHERE league_id = $1 AND user_id = $2",
    )
    .bind(league_id)
    .bind(requester)
    .fetch_one(pool)
    .await?;

    if requester_is_member == 0 {
        return Err(AppError::Forbidden(
            "You are not a member of this league".to_string(),
        ));
    }

    let target_is_member = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM league_members WHERE league_id = $1 AND user_id = $2",
    )
    .bind(league_id)
    .bind(target)
    .fetch_one(pool)
    .await?;

    if target_is_member == 0 {
        return Err(AppError::NotFound(
            "User is not a member of this league".to_string(),
        ));
    }

    Ok(())
}

/// GET /api/leagues/:league_id/members/:user_id/gameweek/:week
///
/// One manager's gameweek, read from that week's lineup snapshot: starters,
/// bench, captain, chip, and the arithmetic behind every player's points.
///
/// Open to any league member once the week has been scored. Before that the
/// same rule as the live lineup view applies — nothing until the lock.
pub async fn get_member_gameweek(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((league_id, target_user_id, week_number)): Path<(Uuid, Uuid, i32)>,
) -> AppResult<Json<MemberGameweekResponse>> {
    require_both_in_league(&state.pool, league_id, auth.user_id, target_user_id).await?;

    let week = readable_week(&state.pool, week_number, auth.user_id == target_user_id).await?;
    let week_id = week.0;

    #[derive(sqlx::FromRow)]
    struct TeamRow {
        id: Uuid,
        name: String,
        username: String,
    }

    let team = sqlx::query_as::<_, TeamRow>(
        "SELECT ft.id, ft.name, u.username
         FROM fantasy_teams ft
         INNER JOIN users u ON u.id = ft.user_id
         WHERE ft.user_id = $1",
    )
    .bind(target_user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("This player hasn't created a team yet".to_string()))?;

    // The snapshot is the only source for a past week. When there is none the
    // lineup is genuinely unknown and we say so, rather than showing the squad
    // they happen to hold today.
    let captain_id = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT COALESCE(tgl.captain_id, ft.captain_id)
         FROM team_gameweek_lineups tgl
         INNER JOIN fantasy_teams ft ON ft.id = tgl.team_id
         WHERE tgl.team_id = $1 AND tgl.match_week_id = $2",
    )
    .bind(team.id)
    .bind(week_id)
    .fetch_optional(&state.pool)
    .await?;

    let has_snapshot = captain_id.is_some();
    let captain_id = captain_id.flatten();

    let lines = fetch_gameweek_lines(&state.pool, team.id, &week).await?;

    let (bench, starters): (Vec<_>, Vec<_>) = lines.into_iter().partition(|l| l.is_bench);

    let chip_played = sqlx::query_scalar::<_, String>(
        "SELECT chip_type FROM team_chips WHERE team_id = $1 AND match_week_id = $2",
    )
    .bind(team.id)
    .bind(week_id)
    .fetch_optional(&state.pool)
    .await?;

    #[derive(sqlx::FromRow)]
    struct StoredScore {
        gross_points: i32,
        transfer_points_hit: i32,
        total_points: i32,
    }

    let stored = sqlx::query_as::<_, StoredScore>(
        "SELECT gross_points, transfer_points_hit, total_points
         FROM team_gameweek_points WHERE team_id = $1 AND match_week_id = $2",
    )
    .bind(team.id)
    .bind(week_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(MemberGameweekResponse {
        user_id: target_user_id,
        username: team.username,
        team_name: team.name,
        week_number,
        has_snapshot,
        captain_id,
        chip_played,
        gross_points: stored.as_ref().map(|s| s.gross_points),
        transfer_points_hit: stored.as_ref().map(|s| s.transfer_points_hit),
        total_points: stored.as_ref().map(|s| s.total_points),
        starters,
        bench,
    }))
}

/// GET /api/leagues/:league_id/gameweek/:week/scoreboard
///
/// Who scored what in one gameweek, for every manager in the league. Each row
/// carries `has_snapshot`, which says whether that manager's lineup can be
/// opened — older weeks predate snapshots and cannot be reconstructed.
pub async fn get_gameweek_scoreboard(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((league_id, week_number)): Path<(Uuid, i32)>,
) -> AppResult<Json<GameweekScoreboard>> {
    require_both_in_league(&state.pool, league_id, auth.user_id, auth.user_id).await?;

    let (week_id, access) = week_state(&state.pool, week_number).await?;

    let entries = sqlx::query_as::<_, GameweekScoreboardEntry>(
        r#"SELECT u.id AS user_id, u.username, u.full_name,
                  ft.name AS team_name,
                  tgp.gross_points, tgp.transfer_points_hit, tgp.total_points,
                  (SELECT tc.chip_type FROM team_chips tc
                   WHERE tc.team_id = ft.id AND tc.match_week_id = $2) AS chip_played,
                  EXISTS (SELECT 1 FROM team_gameweek_lineups tgl
                          WHERE tgl.team_id = ft.id AND tgl.match_week_id = $2) AS has_snapshot
           FROM league_members lm
           INNER JOIN users u ON u.id = lm.user_id
           LEFT JOIN fantasy_teams ft ON ft.user_id = u.id
           LEFT JOIN team_gameweek_points tgp
             ON tgp.team_id = ft.id AND tgp.match_week_id = $2
           WHERE lm.league_id = $1
           ORDER BY tgp.total_points DESC NULLS LAST, u.username"#,
    )
    .bind(league_id)
    .bind(week_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(GameweekScoreboard {
        league_id,
        week_number,
        is_complete: access == LineupAccess::Completed,
        entries,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> Option<sqlx::PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        sqlx::PgPool::connect(&url).await.ok()
    }

    /// Insert a gameweek in a range no real data uses, and say whether it is the
    /// league's open week.
    async fn week(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        number: i32,
        is_active: bool,
    ) -> (Uuid, i32) {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO match_weeks (week_number, start_date, end_date, is_active)
             VALUES ($1, '2099-01-05'::date, '2099-01-11'::date, $2) RETURNING id",
        )
        .bind(number)
        .bind(is_active)
        .fetch_one(&mut **tx)
        .await
        .expect("insert week");
        (id, number)
    }

    /// Store a team score for the week, which is what marks it complete.
    async fn score_the_week(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, week_id: Uuid) {
        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (username, email, password_hash, full_name)
             VALUES ('privacy_probe', 'privacy_probe@example.test', 'x', 'Privacy Probe')
             RETURNING id",
        )
        .fetch_one(&mut **tx)
        .await
        .expect("insert user");

        let team_id: Uuid = sqlx::query_scalar(
            "INSERT INTO fantasy_teams (user_id, name) VALUES ($1, 'Privacy FC') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await
        .expect("insert team");

        sqlx::query(
            "INSERT INTO team_gameweek_points
               (team_id, match_week_id, gross_points, transfer_points_hit, total_points)
             VALUES ($1, $2, 40, 0, 40)",
        )
        .bind(team_id)
        .bind(week_id)
        .execute(&mut **tx)
        .await
        .expect("store score");
    }

    /// The rule managers were promised: nobody sees anybody else's picks for the
    /// gameweek they are still picking. This is the test that has to keep
    /// passing — the rest of the feature is built on top of it.
    #[tokio::test]
    async fn a_rival_cannot_read_the_current_unlocked_gameweek() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        // The open gameweek, before the lock: managers are still choosing.
        let (_id, number) = week(&mut tx, 9810, true).await;
        let (week_id, access) = week_access(&mut tx, number, false)
            .await
            .expect("resolve week");
        assert_eq!(access, LineupAccess::OpenForSelection);

        let refused = clear_to_read(week_id, access, false);
        assert!(
            matches!(refused, Err(AppError::Forbidden(_))),
            "a rival must be refused the current unlocked gameweek, got {:?}",
            refused.err().map(|e| e.to_string()),
        );

        // And the refusal is a 403, not a 404 that would imply it does not exist
        // nor a 400 that would imply the request was malformed.
        let status = clear_to_read(week_id, access, false)
            .err()
            .expect("refusal")
            .into_response()
            .status();
        assert_eq!(status, axum::http::StatusCode::FORBIDDEN);

        // The owner still sees their own.
        assert!(
            clear_to_read(week_id, access, true).is_ok(),
            "a manager must always be able to read their own lineup"
        );

        tx.rollback().await.expect("rollback");
    }

    /// Once the lock takes effect the same week opens up, which is the existing
    /// behaviour this feature had to preserve rather than replace.
    #[tokio::test]
    async fn the_lock_opens_the_current_gameweek_to_the_league() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        let (_id, number) = week(&mut tx, 9811, true).await;
        let (week_id, access) = week_access(&mut tx, number, true)
            .await
            .expect("resolve week");

        assert_eq!(access, LineupAccess::LockedInProgress);
        assert!(clear_to_read(week_id, access, false).is_ok());

        tx.rollback().await.expect("rollback");
    }

    /// A completed gameweek is open to the whole league, lock or no lock.
    #[tokio::test]
    async fn a_completed_gameweek_is_open_to_the_league() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        // Still flagged active and unlocked, to prove it is the score that opens
        // it and not the lock: a week that has been scored is settled.
        let (id, number) = week(&mut tx, 9812, true).await;
        score_the_week(&mut tx, id).await;

        let (week_id, access) = week_access(&mut tx, number, false)
            .await
            .expect("resolve week");

        assert_eq!(access, LineupAccess::Completed);
        assert!(clear_to_read(week_id, access, false).is_ok());

        tx.rollback().await.expect("rollback");
    }

    /// A gameweek nobody has played is not a peephole into anyone's squad.
    #[tokio::test]
    async fn an_unplayed_gameweek_is_refused() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        let (_id, number) = week(&mut tx, 9813, false).await;
        let (week_id, access) = week_access(&mut tx, number, false)
            .await
            .expect("resolve week");

        assert_eq!(access, LineupAccess::NotPlayed);
        assert!(matches!(
            clear_to_read(week_id, access, false),
            Err(AppError::Forbidden(_))
        ));

        tx.rollback().await.expect("rollback");
    }
}
