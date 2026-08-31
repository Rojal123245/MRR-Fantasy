//! Scoring one team's gameweek.
//!
//! The per-gameweek engine lives here rather than inline in the admin handler so
//! that tests score a week through exactly the code production runs. A test that
//! re-implemented this loop could agree with itself while disagreeing with the
//! handler, which is the whole class of bug this module exists to rule out.
//!
//! All SQL is composed in [`super::points_sql`]; nothing here spells out scoring.

use uuid::Uuid;

use super::deadline;
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

/// Open a gameweek: make it the live one, give it a deadline, and seed a
/// lineup for every team.
///
/// The three parts belong together. A week that is live without a deadline
/// accepts nothing — every guard reads a missing deadline as closed — and a
/// week without seeded lineups is invisible to
/// [`points_sql::scored_teams`]'s eligibility test for any manager who never
/// opens the app, which would drop them from the week entirely.
///
/// `deadline` is `Some` only when the caller means "this week starts now, set
/// its deadline outright" — which is creating it. Activating an existing week
/// passes `None`, and then:
///
///   * a scored week is never touched. Its deadline is the record of what it
///     was played to, and nothing can recompute it — migration 021's backfill
///     only fills a deadline that is NULL — so an admin toggling an old week to
///     look at it must not erase that.
///   * a deadline still in the future is kept. Re-scoring an old gameweek
///     re-activates the live one, and that must not push the live week's
///     deadline a week further out in the middle of the week.
///   * a deadline already passed is replaced. Keeping it would open the week
///     permanently sealed: every save silently dropped, every manager scored on
///     a squad from whenever the week was last touched, and no way to tell from
///     the admin console. A week created early, parked, and activated weeks
///     later is exactly this case.
///
/// The one thing that slips through is re-activating a week that was played but
/// never scored, which gets a fresh deadline it arguably should not. That is a
/// deliberate admin action on a week they have chosen not to settle, and a
/// visibly extended deadline is a better failure than a silently dead one.
pub async fn open_week(
    conn: &mut sqlx::PgConnection,
    week_id: Uuid,
    deadline: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE match_weeks w
           SET is_active = true,
               lineup_deadline = CASE
                 WHEN EXISTS (
                   SELECT 1 FROM team_gameweek_points g WHERE g.match_week_id = w.id
                 ) THEN w.lineup_deadline
                 ELSE COALESCE(
                   $2,
                   CASE WHEN w.lineup_deadline > NOW() THEN w.lineup_deadline END,
                   $3
                 )
               END
           WHERE w.id = $1"#,
    )
    .bind(week_id)
    .bind(deadline)
    .bind(deadline::next_deadline())
    .execute(&mut *conn)
    .await?;

    snapshot_all_lineups(&mut *conn, week_id).await
}

/// Seed every team's squad into a gameweek.
///
/// Used when a gameweek opens, so an open week always has a lineup for every
/// team from the outset — not only for the managers who happen to touch their
/// team before it is scored. What each manager actually plays is written by
/// [`refresh_team_lineup`] as they change it, up to the deadline.
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

    open_week(&mut *conn, next_id, None).await?;

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

/// Seed a team's lineup for a gameweek from the squad it holds right now.
///
/// This is the opening position, written for every team when a gameweek opens,
/// so a manager who never touches their team still has a lineup to be scored
/// from. It is *not* the final word: [`refresh_team_lineup`] rewrites it on
/// every squad change until the week's deadline, which is what makes the
/// lineup the one the manager actually played.
///
/// The first seed for a team+week wins, and the player rows are written only
/// when the lineup row is created. Re-populating them from here is not made
/// safe by `ON CONFLICT DO NOTHING`: that clause protects the rows already
/// there, but still *appends* squad members who were not in the original
/// freeze, which is how 60 rows came to sit in gameweeks 1-3 and had to be
/// pruned by hand. Replacing the squad is `refresh_team_lineup`'s job, and it
/// deletes before it inserts.
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

/// What a refresh did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum LineupFreeze {
    /// The snapshot now holds exactly the squad in `team_players`.
    Refreshed,
    /// The gameweek is past its deadline, already scored, or has no deadline
    /// recorded. The snapshot was left exactly as it was.
    Sealed,
}

/// Rewrite a team's lineup for a gameweek from the squad it holds now, if that
/// gameweek is still open to changes.
///
/// A gameweek is scored from its lineup snapshot, so the snapshot has to be the
/// squad the manager had at the deadline — not the one they happened to hold
/// when the week opened, which is a week earlier and is what
/// [`snapshot_team_lineup`] writes. Every squad change therefore rewrites it,
/// until the deadline passes and the week seals itself.
///
/// There is no scheduler, and none is needed: the snapshot is by construction
/// the squad as of the last write that beat the deadline, which is the squad at
/// the deadline.
///
/// Call this **after** the squad mutation and **on the same transaction**. The
/// snapshot is then a copy of what was actually committed, and either both land
/// or neither does.
///
/// The player rows are DELETEd before they are INSERTed. Appending — which is
/// what `ON CONFLICT DO NOTHING` would do here — is the defect that put 60
/// extra players into gameweeks 1-3 and paid managers for players they had not
/// picked; see `ops/2026-08-24_repair_scored_weeks.sql`.
pub async fn refresh_team_lineup(
    conn: &mut sqlx::PgConnection,
    team_id: Uuid,
    week_id: Uuid,
) -> Result<LineupFreeze, sqlx::Error> {
    // Wait for any scoring run on this week to finish before touching its
    // lineup. See `lock_week_for_scoring`: without this a save can land in the
    // middle of the scoring loop and be half-counted.
    //
    // Shared, so saves never block each other, and taken first so this
    // transaction's lock order matches scoring's.
    sqlx::query("SELECT id FROM match_weeks WHERE id = $1 FOR SHARE")
        .bind(week_id)
        .execute(&mut *conn)
        .await?;

    // The gate lives in the same statement as the write, so a save racing the
    // deadline cannot pass a separate check and then land on the other side of
    // it. `NOW()` is the transaction timestamp, so every statement in the
    // caller's transaction agrees on which side of the deadline it is on.
    //
    // `NOW() < w.lineup_deadline` is NULL, not true, for a week with no
    // deadline recorded, so such a week seals rather than reopening.
    //
    // `DO UPDATE` rather than `DO NOTHING`: the captain must move with the
    // squad, and it makes an empty result mean one thing only — sealed.
    //
    // A team with no squad yet gets nothing at all. A header with no players
    // under it is worse than no header: `points_sql::week_is_scored` reads
    // `tgl.id IS NOT NULL` as "this team played the week", so it would be
    // scored 0 from an empty snapshot instead of being skipped. There is one
    // such row in production, on gameweek 2, from a manager who joined
    // mid-week.
    let lineup_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO team_gameweek_lineups (team_id, match_week_id, captain_id)
           SELECT ft.id, w.id, ft.captain_id
           FROM match_weeks w
           CROSS JOIN fantasy_teams ft
           WHERE w.id = $2
             AND ft.id = $1
             AND NOW() < w.lineup_deadline
             AND NOT EXISTS (
               SELECT 1 FROM team_gameweek_points g WHERE g.match_week_id = w.id
             )
             AND EXISTS (SELECT 1 FROM team_players tp WHERE tp.team_id = ft.id)
           ON CONFLICT (team_id, match_week_id) DO UPDATE
             SET captain_id = EXCLUDED.captain_id
           RETURNING id"#,
    )
    .bind(team_id)
    .bind(week_id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some(lineup_id) = lineup_id else {
        return Ok(LineupFreeze::Sealed);
    };

    sqlx::query("DELETE FROM team_gameweek_lineup_players WHERE team_gameweek_lineup_id = $1")
        .bind(lineup_id)
        .execute(&mut *conn)
        .await?;

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

    Ok(LineupFreeze::Refreshed)
}

/// Take a gameweek for scoring, excluding every concurrent lineup refresh.
///
/// Scoring reads a team's lineup over several statements. Under READ COMMITTED
/// each one takes a fresh snapshot, so a save committing partway through is
/// half-counted and the stored score reproduces from no lineup at all. The
/// already-scored guard cannot prevent it: this transaction's
/// `team_gameweek_points` rows are invisible outside it until it commits.
///
/// [`refresh_team_lineup`] takes the same row `FOR SHARE`, so saves queue
/// behind a scoring run and never block one another.
///
/// Call this from inside the scoring transaction, after any write to
/// `fantasy_teams`, so the lock order is the same on both sides.
pub async fn lock_week_for_scoring(
    conn: &mut sqlx::PgConnection,
    week_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT id FROM match_weeks WHERE id = $1 FOR UPDATE")
        .bind(week_id)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

/// Whether a gameweek is still open to squad changes aimed at it.
///
/// False once the deadline has passed, once the week is scored, and for a week
/// with no deadline recorded. Transfers and chips are recorded *against* a
/// gameweek, so they must be refused when this is false — otherwise they land
/// on a week that has already been played, where a transfer still charges its
/// -4 hit and a chip pays nothing.
pub async fn week_accepts_changes(
    conn: &mut sqlx::PgConnection,
    week_id: Uuid,
) -> Result<bool, sqlx::Error> {
    // COALESCE, because `NOW() < NULL` is NULL: a week with no deadline
    // recorded is closed, not open.
    sqlx::query_scalar::<_, bool>(
        r#"SELECT COALESCE(NOW() < w.lineup_deadline, false)
                  AND NOT EXISTS (
                    SELECT 1 FROM team_gameweek_points g WHERE g.match_week_id = w.id
                  )
           FROM match_weeks w WHERE w.id = $1"#,
    )
    .bind(week_id)
    .fetch_optional(&mut *conn)
    .await
    .map(|found| found.unwrap_or(false))
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
