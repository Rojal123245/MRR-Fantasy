use axum::{
    extract::{Extension, Path, State},
    Json,
};
use rand::Rng;
use uuid::Uuid;

use crate::auth::handler::AppState;
use crate::auth::middleware::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateLeagueRequest, JoinLeagueRequest, League, LeagueDetail, LeagueMemberStanding, MyLeague,
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
             -- Starter points (2x captain) + triple captain bonus + bench boost bonus
             COALESCE((
               SELECT SUM(
                 (
                   CASE tp.assigned_position::text
                     WHEN 'GK'  THEN pp.goals * 10
                     WHEN 'DEF' THEN pp.goals * 6
                     WHEN 'MID' THEN pp.goals * 5
                     WHEN 'FWD' THEN pp.goals * 4
                     ELSE 0
                   END
                   + pp.assists * 5
                   + CASE tp.assigned_position::text
                       WHEN 'GK'  THEN pp.clean_sheets * 10
                       WHEN 'DEF' THEN pp.clean_sheets * 6
                       ELSE 0
                     END
                   + CASE WHEN tp.assigned_position::text = 'GK' THEN pp.saves / 5 ELSE 0 END
                   + pp.penalty_saves * 8
                   + CASE WHEN pp.minutes_played >= 35 THEN 2
                          WHEN pp.minutes_played >= 1  THEN 1
                          ELSE 0 END
                   - pp.own_goals * 2
                   - pp.penalty_misses * 2
                   - pp.regular_fouls * 1
                   - pp.serious_fouls * 3
                 )
                 * CASE WHEN tp.player_id = ft.captain_id THEN 2 ELSE 1 END
               )
               FROM team_players tp
               JOIN player_points pp ON pp.player_id = tp.player_id
               WHERE tp.team_id = ft.id AND tp.is_bench = false
             ), 0)
             + COALESCE((
               SELECT SUM(
                 CASE tp2.assigned_position::text
                   WHEN 'GK'  THEN pp2.goals * 10
                   WHEN 'DEF' THEN pp2.goals * 6
                   WHEN 'MID' THEN pp2.goals * 5
                   WHEN 'FWD' THEN pp2.goals * 4
                   ELSE 0
                 END
                 + pp2.assists * 5
                 + CASE tp2.assigned_position::text
                     WHEN 'GK'  THEN pp2.clean_sheets * 10
                     WHEN 'DEF' THEN pp2.clean_sheets * 6
                     ELSE 0
                   END
                 + CASE WHEN tp2.assigned_position::text = 'GK' THEN pp2.saves / 5 ELSE 0 END
                 + pp2.penalty_saves * 8
                 + CASE WHEN pp2.minutes_played >= 35 THEN 2
                        WHEN pp2.minutes_played >= 1  THEN 1
                        ELSE 0 END
                 - pp2.own_goals * 2
                 - pp2.penalty_misses * 2
                 - pp2.regular_fouls * 1
                 - pp2.serious_fouls * 3
               )
               FROM team_chips tc2
               JOIN team_players tp2 ON tp2.team_id = ft.id
                 AND tp2.player_id = ft.captain_id AND tp2.is_bench = false
               JOIN player_points pp2 ON pp2.player_id = tp2.player_id
                 AND pp2.match_week_id = tc2.match_week_id
               WHERE tc2.team_id = ft.id AND tc2.chip_type = 'triple_captain'
             ), 0)
             + COALESCE((
               SELECT SUM(
                 CASE p3.position::text
                   WHEN 'GK'  THEN pp3.goals * 10
                   WHEN 'DEF' THEN pp3.goals * 6
                   WHEN 'MID' THEN pp3.goals * 5
                   WHEN 'FWD' THEN pp3.goals * 4
                   ELSE 0
                 END
                 + pp3.assists * 5
                 + CASE p3.position::text
                     WHEN 'GK'  THEN pp3.clean_sheets * 10
                     WHEN 'DEF' THEN pp3.clean_sheets * 6
                     ELSE 0
                   END
                 + CASE WHEN p3.position::text = 'GK' THEN pp3.saves / 5 ELSE 0 END
                 + pp3.penalty_saves * 8
                 + CASE WHEN pp3.minutes_played >= 35 THEN 2
                        WHEN pp3.minutes_played >= 1  THEN 1
                        ELSE 0 END
                 - pp3.own_goals * 2
                 - pp3.penalty_misses * 2
                 - pp3.regular_fouls * 1
                 - pp3.serious_fouls * 3
               )
               FROM team_chips tc3
               JOIN team_players tp3 ON tp3.team_id = ft.id AND tp3.is_bench = true
               JOIN players p3 ON p3.id = tp3.player_id
               JOIN player_points pp3 ON pp3.player_id = tp3.player_id
                 AND pp3.match_week_id = tc3.match_week_id
               WHERE tc3.team_id = ft.id AND tc3.chip_type = 'bench_boost'
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
             -- Starter points (2x captain) + triple captain bonus + bench boost bonus
             COALESCE((
               SELECT SUM(
                 (
                   CASE tp.assigned_position::text
                     WHEN 'GK'  THEN pp.goals * 10
                     WHEN 'DEF' THEN pp.goals * 6
                     WHEN 'MID' THEN pp.goals * 5
                     WHEN 'FWD' THEN pp.goals * 4
                     ELSE 0
                   END
                   + pp.assists * 5
                   + CASE tp.assigned_position::text
                       WHEN 'GK'  THEN pp.clean_sheets * 10
                       WHEN 'DEF' THEN pp.clean_sheets * 6
                       ELSE 0
                     END
                   + CASE WHEN tp.assigned_position::text = 'GK' THEN pp.saves / 5 ELSE 0 END
                   + pp.penalty_saves * 8
                   + CASE WHEN pp.minutes_played >= 35 THEN 2
                          WHEN pp.minutes_played >= 1  THEN 1
                          ELSE 0 END
                   - pp.own_goals * 2
                   - pp.penalty_misses * 2
                   - pp.regular_fouls * 1
                   - pp.serious_fouls * 3
                 )
                 * CASE WHEN tp.player_id = ft.captain_id THEN 2 ELSE 1 END
               )
               FROM team_players tp
               JOIN player_points pp ON pp.player_id = tp.player_id
               WHERE tp.team_id = ft.id AND tp.is_bench = false
             ), 0)
             + COALESCE((
               SELECT SUM(
                 CASE tp2.assigned_position::text
                   WHEN 'GK'  THEN pp2.goals * 10
                   WHEN 'DEF' THEN pp2.goals * 6
                   WHEN 'MID' THEN pp2.goals * 5
                   WHEN 'FWD' THEN pp2.goals * 4
                   ELSE 0
                 END
                 + pp2.assists * 5
                 + CASE tp2.assigned_position::text
                     WHEN 'GK'  THEN pp2.clean_sheets * 10
                     WHEN 'DEF' THEN pp2.clean_sheets * 6
                     ELSE 0
                   END
                 + CASE WHEN tp2.assigned_position::text = 'GK' THEN pp2.saves / 5 ELSE 0 END
                 + pp2.penalty_saves * 8
                 + CASE WHEN pp2.minutes_played >= 35 THEN 2
                        WHEN pp2.minutes_played >= 1  THEN 1
                        ELSE 0 END
                 - pp2.own_goals * 2
                 - pp2.penalty_misses * 2
                 - pp2.regular_fouls * 1
                 - pp2.serious_fouls * 3
               )
               FROM team_chips tc2
               JOIN team_players tp2 ON tp2.team_id = ft.id
                 AND tp2.player_id = ft.captain_id AND tp2.is_bench = false
               JOIN player_points pp2 ON pp2.player_id = tp2.player_id
                 AND pp2.match_week_id = tc2.match_week_id
               WHERE tc2.team_id = ft.id AND tc2.chip_type = 'triple_captain'
             ), 0)
             + COALESCE((
               SELECT SUM(
                 CASE p3.position::text
                   WHEN 'GK'  THEN pp3.goals * 10
                   WHEN 'DEF' THEN pp3.goals * 6
                   WHEN 'MID' THEN pp3.goals * 5
                   WHEN 'FWD' THEN pp3.goals * 4
                   ELSE 0
                 END
                 + pp3.assists * 5
                 + CASE p3.position::text
                     WHEN 'GK'  THEN pp3.clean_sheets * 10
                     WHEN 'DEF' THEN pp3.clean_sheets * 6
                     ELSE 0
                   END
                 + CASE WHEN p3.position::text = 'GK' THEN pp3.saves / 5 ELSE 0 END
                 + pp3.penalty_saves * 8
                 + CASE WHEN pp3.minutes_played >= 35 THEN 2
                        WHEN pp3.minutes_played >= 1  THEN 1
                        ELSE 0 END
                 - pp3.own_goals * 2
                 - pp3.penalty_misses * 2
                 - pp3.regular_fouls * 1
                 - pp3.serious_fouls * 3
               )
               FROM team_chips tc3
               JOIN team_players tp3 ON tp3.team_id = ft.id AND tp3.is_bench = true
               JOIN players p3 ON p3.id = tp3.player_id
               JOIN player_points pp3 ON pp3.player_id = tp3.player_id
                 AND pp3.match_week_id = tc3.match_week_id
               WHERE tc3.team_id = ft.id AND tc3.chip_type = 'bench_boost'
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
