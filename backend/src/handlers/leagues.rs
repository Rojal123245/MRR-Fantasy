use axum::{
    extract::{Extension, Path, State},
    Json,
};
use rand::Rng;
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};
use crate::handlers::teams::compute_lock_status;
use crate::models::{
    CreateLeagueRequest, JoinLeagueRequest, League, LeagueDetail, LeagueMemberStanding,
    MemberLineupResponse, MyLeague, Player, PlayerPosition, StarterPlayer,
};

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

#[derive(Debug, sqlx::FromRow)]
struct LineupStarterRow {
    id: Uuid,
    name: String,
    position: PlayerPosition,
    secondary_position: Option<PlayerPosition>,
    is_top_player: bool,
    team_name: String,
    photo_url: Option<String>,
    price: rust_decimal::Decimal,
    total_points: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    assigned_position: Option<PlayerPosition>,
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
            "Lineups are only visible after the gameweek starts (Saturday midnight to Sunday 12:00 PM ET)"
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

    let starter_rows = sqlx::query_as::<_, LineupStarterRow>(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player,
                  p.team_name, p.photo_url, p.price, p.created_at,
                  tp.assigned_position,
                  COALESCE((
                    SELECT SUM(
                      CASE COALESCE(tp.assigned_position, p.position)::text
                        WHEN 'GK'  THEN pp.goals * 10
                        WHEN 'DEF' THEN pp.goals * 6
                        WHEN 'MID' THEN pp.goals * 5
                        WHEN 'FWD' THEN pp.goals * 4
                        ELSE 0
                      END
                      + pp.assists * 5
                      + CASE COALESCE(tp.assigned_position, p.position)::text
                          WHEN 'GK'  THEN pp.clean_sheets * 10
                          WHEN 'DEF' THEN pp.clean_sheets * 6
                          ELSE 0
                        END
                      + CASE WHEN COALESCE(tp.assigned_position, p.position)::text = 'GK' THEN pp.saves / 5 ELSE 0 END
                      + pp.penalty_saves * 8
                      + CASE WHEN pp.minutes_played >= 35 THEN 2
                             WHEN pp.minutes_played >= 1  THEN 1
                             ELSE 0 END
                      - pp.own_goals * 2
                      - pp.penalty_misses * 2
                      - pp.regular_fouls * 1
                      - pp.serious_fouls * 3
                    )
                    FROM player_points pp
                    WHERE pp.player_id = p.id
                  ), 0)::int AS total_points
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           WHERE tp.team_id = $1 AND tp.is_bench = false"#,
    )
    .bind(team.id)
    .fetch_all(&state.pool)
    .await?;

    let starters: Vec<StarterPlayer> = starter_rows
        .into_iter()
        .map(|r| {
            let assigned = r.assigned_position.unwrap_or_else(|| r.position.clone());
            StarterPlayer {
                player: Player {
                    id: r.id,
                    name: r.name,
                    position: r.position,
                    secondary_position: r.secondary_position,
                    is_top_player: r.is_top_player,
                    team_name: r.team_name,
                    photo_url: r.photo_url,
                    price: r.price,
                    total_points: r.total_points,
                    created_at: r.created_at,
                },
                assigned_position: assigned,
            }
        })
        .collect();

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
