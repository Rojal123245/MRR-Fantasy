//! Scoring one team's gameweek.
//!
//! The per-gameweek engine lives here rather than inline in the admin handler so
//! that tests score a week through exactly the code production runs. A test that
//! re-implemented this loop could agree with itself while disagreeing with the
//! handler, which is the whole class of bug this module exists to rule out.
//!
//! All SQL is composed in [`super::points_sql`]; nothing here spells out scoring.

use uuid::Uuid;

use super::points_sql::{self, Source};

/// One team's score for one gameweek, broken down far enough to explain it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamWeekScore {
    /// Starters' points, each scored on the role they were played in.
    pub starter_base: i64,
    /// The captain's own points, already counted once inside `starter_base`.
    pub captain_points: i32,
    /// What the captaincy adds on top: 1x `captain_points`, or 2x under Triple
    /// Captain, making the captain worth 2x or 3x in total.
    pub captain_bonus: i64,
    /// Bench points, counted only when Bench Boost was played.
    pub bench_bonus: i64,
    pub triple_captain: bool,
    pub bench_boost: bool,
    pub transfers: i64,
    pub transfer_points_hit: i32,
    pub gross_points: i32,
    pub total_points: i32,
}

/// Score one team's gameweek from its lineup snapshot, or from the live squad
/// when that week has no snapshot.
///
/// `lineup_id` and `captain_id` come from [`points_sql::scored_teams`]. Passing
/// `lineup_id` keeps a scored week immutable: once a snapshot exists, later
/// transfers and re-arrangements cannot change what that week paid.
pub async fn score_team_gameweek(
    conn: &mut sqlx::PgConnection,
    team_id: Uuid,
    lineup_id: Option<Uuid>,
    captain_id: Option<Uuid>,
    week_id: Uuid,
) -> Result<TeamWeekScore, sqlx::Error> {
    let (source, source_id) = match lineup_id {
        Some(lineup_id) => (Source::Snapshot, lineup_id),
        None => (Source::LiveSquad, team_id),
    };

    let starter_base = sqlx::query_scalar::<_, i64>(&points_sql::squad_half_total(source, false))
        .bind(source_id)
        .bind(week_id)
        .fetch_one(&mut *conn)
        .await?;

    let triple_captain = chip_played(&mut *conn, team_id, week_id, "triple_captain").await?;
    let bench_boost = chip_played(&mut *conn, team_id, week_id, "bench_boost").await?;

    // The captain is already counted once in `starter_base`, so adding his score
    // again makes 2x, and twice again makes 3x under Triple Captain.
    let captain_points = match captain_id {
        Some(captain_id) => sqlx::query_scalar::<_, i32>(&points_sql::single_starter_total(source))
            .bind(source_id)
            .bind(week_id)
            .bind(captain_id)
            .fetch_optional(&mut *conn)
            .await?
            .unwrap_or(0),
        None => 0,
    };
    let captain_bonus = if triple_captain {
        (captain_points * 2) as i64
    } else {
        captain_points as i64
    };

    let bench_bonus = if bench_boost {
        sqlx::query_scalar::<_, i64>(&points_sql::squad_half_total(source, true))
            .bind(source_id)
            .bind(week_id)
            .fetch_one(&mut *conn)
            .await?
    } else {
        0
    };

    let transfers = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM transfers WHERE team_id = $1 AND match_week_id = $2",
    )
    .bind(team_id)
    .bind(week_id)
    .fetch_one(&mut *conn)
    .await?;

    let transfer_points_hit = ((transfers as i32) - 1).max(0) * 4;
    let gross_points = (starter_base + captain_bonus + bench_bonus) as i32;

    Ok(TeamWeekScore {
        starter_base,
        captain_points,
        captain_bonus,
        bench_bonus,
        triple_captain,
        bench_boost,
        transfers,
        transfer_points_hit,
        gross_points,
        total_points: gross_points - transfer_points_hit,
    })
}

/// Persist a scored gameweek, replacing any previous score for that team+week.
pub async fn store_team_gameweek_score(
    conn: &mut sqlx::PgConnection,
    team_id: Uuid,
    week_id: Uuid,
    score: &TeamWeekScore,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO team_gameweek_points
             (team_id, match_week_id, gross_points, transfer_points_hit, total_points)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (team_id, match_week_id) DO UPDATE SET
             gross_points = EXCLUDED.gross_points,
             transfer_points_hit = EXCLUDED.transfer_points_hit,
             total_points = EXCLUDED.total_points,
             updated_at = NOW()"#,
    )
    .bind(team_id)
    .bind(week_id)
    .bind(score.gross_points)
    .bind(score.transfer_points_hit)
    .bind(score.total_points)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Freeze every team's squad into a gameweek.
///
/// Used both when a gameweek is created and when scoring the previous one opens
/// it, so an open week always has a lineup for every team from the outset —
/// not only for the managers who happen to touch their team before it is scored.
pub async fn snapshot_all_lineups(
    conn: &mut sqlx::PgConnection,
    week_id: Uuid,
) -> Result<(), sqlx::Error> {
    let team_ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM fantasy_teams")
        .fetch_all(&mut *conn)
        .await?;

    for team_id in team_ids {
        snapshot_team_lineup(&mut *conn, team_id, week_id).await?;
    }

    Ok(())
}

/// Close a gameweek that has just been scored and open the next one.
///
/// A scored week is over: leaving it open lets chips, transfers and lineup
/// changes keep landing on a week whose points are already stored, where they
/// can no longer affect anything. Closing it without opening a successor would
/// leave managers with nowhere to play, so the two happen together.
///
/// Does nothing unless the week is the one currently open — re-running an older
/// gameweek to correct it must not wind the league back to that point.
///
/// Returns the week number now open, or `None` if the league did not move: the
/// scored week was not the open one, or it was the final gameweek.
pub async fn close_week_and_open_next(
    conn: &mut sqlx::PgConnection,
    week_id: Uuid,
    week_number: i32,
    week_is_open: bool,
) -> Result<Option<i32>, sqlx::Error> {
    if !week_is_open {
        return Ok(None);
    }

    sqlx::query("UPDATE match_weeks SET is_active = false WHERE id = $1")
        .bind(week_id)
        .execute(&mut *conn)
        .await?;

    let next = sqlx::query_as::<_, (Uuid, i32)>(
        r#"SELECT w.id, w.week_number
           FROM match_weeks w
           WHERE w.week_number > $1
             AND NOT EXISTS (
               SELECT 1 FROM team_gameweek_points g WHERE g.match_week_id = w.id
             )
           ORDER BY w.week_number
           LIMIT 1"#,
    )
    .bind(week_number)
    .fetch_optional(&mut *conn)
    .await?;

    let Some((next_id, next_number)) = next else {
        return Ok(None);
    };

    sqlx::query("UPDATE match_weeks SET is_active = true WHERE id = $1")
        .bind(next_id)
        .execute(&mut *conn)
        .await?;

    snapshot_all_lineups(&mut *conn, next_id).await?;

    Ok(Some(next_number))
}

/// Whether a gameweek has already been scored.
///
/// A scored week is closed: its lineups are frozen and its points are stored.
/// Chips, transfers and lineup changes aimed at it can no longer affect what it
/// paid, so accepting them silently loses the manager's move. `submit_week_stats`
/// deactivates a week as it scores it, which normally puts a scored week out of
/// reach; this is the second line of defence for a week that is re-activated or
/// re-scored.
pub async fn week_already_scored(
    conn: &mut sqlx::PgConnection,
    week_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM team_gameweek_points WHERE match_week_id = $1)",
    )
    .bind(week_id)
    .fetch_one(&mut *conn)
    .await
}

/// Freeze a team's current squad as its lineup for a gameweek.
///
/// The first snapshot for a team+week wins. Everything after it — a transfer, a
/// re-arrangement, a change of captain — must leave the frozen lineup alone,
/// because that lineup is what the week is scored from.
///
/// The player rows are therefore written only when the lineup row is created.
/// Re-populating them on every call is not made safe by `ON CONFLICT DO
/// NOTHING`: that clause protects the rows already there, but still *appends*
/// squad members who were not in the original freeze. Because the handlers call
/// this before every lineup change and every transfer, a week left active would
/// otherwise accumulate the union of every squad the manager held during it, and
/// re-scoring that week would pay all of them.
///
/// Returns the lineup id, whether it already existed or was created here.
pub async fn snapshot_team_lineup(
    conn: &mut sqlx::PgConnection,
    team_id: Uuid,
    week_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let captain_id =
        sqlx::query_scalar::<_, Option<Uuid>>("SELECT captain_id FROM fantasy_teams WHERE id = $1")
            .bind(team_id)
            .fetch_optional(&mut *conn)
            .await?
            .flatten();

    // `DO NOTHING` rather than `DO UPDATE`, so a row comes back only when this
    // call is the one that froze the week.
    let freshly_frozen = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO team_gameweek_lineups (team_id, match_week_id, captain_id)
           VALUES ($1, $2, $3)
           ON CONFLICT (team_id, match_week_id) DO NOTHING
           RETURNING id"#,
    )
    .bind(team_id)
    .bind(week_id)
    .bind(captain_id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some(lineup_id) = freshly_frozen else {
        // Already frozen. Leave it exactly as it was.
        return sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM team_gameweek_lineups WHERE team_id = $1 AND match_week_id = $2",
        )
        .bind(team_id)
        .bind(week_id)
        .fetch_one(&mut *conn)
        .await;
    };

    sqlx::query(
        r#"INSERT INTO team_gameweek_lineup_players
             (team_gameweek_lineup_id, player_id, is_bench, assigned_position)
           SELECT $1, tp.player_id, tp.is_bench, tp.assigned_position
           FROM team_players tp
           WHERE tp.team_id = $2"#,
    )
    .bind(lineup_id)
    .bind(team_id)
    .execute(&mut *conn)
    .await?;

    Ok(lineup_id)
}

async fn chip_played(
    conn: &mut sqlx::PgConnection,
    team_id: Uuid,
    week_id: Uuid,
    chip_type: &str,
) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_chips
         WHERE team_id = $1 AND match_week_id = $2 AND chip_type = $3",
    )
    .bind(team_id)
    .bind(week_id)
    .bind(chip_type)
    .fetch_one(&mut *conn)
    .await?;

    Ok(count > 0)
}
