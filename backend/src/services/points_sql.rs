//! Canonical SQL for MRR Fantasy scoring.
//!
//! This is the SQL counterpart to [`super::points_engine::PointsEngine`] and must
//! stay in lockstep with it; `tests::sql_matches_rust_engine` enforces that.
//!
//! Every scoring query lives here rather than inline at the call sites. The
//! expression used to be copy-pasted across eight queries, which is how starters
//! came to be scored on their assigned position while bench-boosted players were
//! still scored on their primary one.

/// SQL expression yielding the position a squad member should be scored as.
///
/// Points depend on the role the manager assigned, not the player's natural
/// position, so a forward played at the back earns a defender's rates. `alias` is
/// the table holding `assigned_position` (`team_players` or
/// `team_gameweek_lineup_players`), and `p` must be the `players` row.
fn scoring_position(alias: &str) -> String {
    format!("COALESCE({alias}.assigned_position, p.position)::text")
}

/// SQL expression for one `player_points` row's fantasy points.
///
/// `position` must be a `::text` position expression; `pp` is the alias of the
/// `player_points` row. Parenthesised so it is safe to multiply or aggregate.
fn week_points(position: &str, pp: &str) -> String {
    format!(
        r#"(
          CASE {position}
            WHEN 'GK'  THEN COALESCE({pp}.goals, 0) * 10
            WHEN 'DEF' THEN COALESCE({pp}.goals, 0) * 6
            WHEN 'MID' THEN COALESCE({pp}.goals, 0) * 5
            WHEN 'FWD' THEN COALESCE({pp}.goals, 0) * 4
            ELSE 0
          END
          + COALESCE({pp}.assists, 0) * 5
          + CASE {position}
              WHEN 'GK'  THEN COALESCE({pp}.clean_sheets, 0) * 2
              WHEN 'DEF' THEN COALESCE({pp}.clean_sheets, 0) * 2
              ELSE 0
            END
          + CASE WHEN {position} = 'GK' THEN COALESCE({pp}.saves, 0) / 5 ELSE 0 END
          + COALESCE({pp}.penalty_saves, 0) * 8
          + CASE WHEN COALESCE({pp}.minutes_played, 0) >= 35 THEN 2
                 WHEN COALESCE({pp}.minutes_played, 0) >= 1  THEN 1
                 ELSE 0 END
          - COALESCE({pp}.own_goals, 0) * 2
          - COALESCE({pp}.penalty_misses, 0) * 2
          - COALESCE({pp}.regular_fouls, 0) * 1
          - COALESCE({pp}.serious_fouls, 0) * 3
        )"#
    )
}

/// SQL expression for the captain multiplier on a single gameweek: 2x as
/// captain, 3x when Triple Captain was played that week, otherwise 1x.
///
/// Resolves the captain the way scoring does — that gameweek's lineup snapshot
/// first, falling back to the team's current captain when the snapshot has none.
/// Expects `tgl`, `ft`, `p` and `pp` in scope.
fn captain_multiplier() -> &'static str {
    r#"CASE
         WHEN COALESCE(tgl.captain_id, ft.captain_id) = p.id THEN
           CASE WHEN EXISTS (
             SELECT 1 FROM team_chips tc
             WHERE tc.team_id = ft.id
               AND tc.match_week_id = pp.match_week_id
               AND tc.chip_type = 'triple_captain'
           ) THEN 3 ELSE 2 END
         ELSE 1
       END"#
}

/// Which squad table a gameweek is scored from.
///
/// Once a gameweek has a lineup snapshot, that snapshot is the source of truth so
/// later transfers cannot retroactively change a scored week. Teams with no
/// snapshot fall back to their current squad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `team_gameweek_lineup_players` for a given `team_gameweek_lineups.id`.
    Snapshot,
    /// `team_players` for a given `fantasy_teams.id`.
    LiveSquad,
}

impl Source {
    fn alias(self) -> &'static str {
        match self {
            Source::Snapshot => "tglp",
            Source::LiveSquad => "tp",
        }
    }

    /// FROM/JOIN clauses binding `$1` to the lineup or team id and `$2` to the week.
    fn squad_join(self) -> &'static str {
        match self {
            Source::Snapshot => {
                r#"FROM team_gameweek_lineup_players tglp
                   JOIN players p ON p.id = tglp.player_id
                   LEFT JOIN player_points pp ON pp.player_id = tglp.player_id AND pp.match_week_id = $2
                   WHERE tglp.team_gameweek_lineup_id = $1"#
            }
            Source::LiveSquad => {
                r#"FROM team_players tp
                   JOIN players p ON p.id = tp.player_id
                   LEFT JOIN player_points pp ON pp.player_id = tp.player_id AND pp.match_week_id = $2
                   WHERE tp.team_id = $1"#
            }
        }
    }
}

/// Sum of a gameweek's points for one half of a squad, before the captain bonus.
///
/// Binds `$1` = lineup or team id, `$2` = match week id. Returns `bigint`.
pub fn squad_half_total(source: Source, is_bench: bool) -> String {
    let alias = source.alias();
    format!(
        "SELECT COALESCE(SUM({points}), 0) {from} AND {alias}.is_bench = {is_bench}",
        points = week_points(&scoring_position(alias), "pp"),
        from = source.squad_join(),
    )
}

/// A single starter's points for a gameweek, used to compute the captain bonus.
///
/// Binds `$1` = lineup or team id, `$2` = match week id, `$3` = player id.
/// Returns `int`, or no row when that player did not start.
pub fn single_starter_total(source: Source) -> String {
    let alias = source.alias();
    format!(
        "SELECT COALESCE({points}, 0) {from} AND {alias}.is_bench = false AND {alias}.player_id = $3",
        points = week_points(&scoring_position(alias), "pp"),
        from = source.squad_join(),
    )
}

/// Teams eligible to be scored for a gameweek.
///
/// A team qualifies if it has a lineup snapshot for that week, or if it already
/// existed when the week ended. Without the second test, re-running an old
/// gameweek would fall back to the live squad for managers who joined later and
/// retroactively award them points for a week they never played. The `created_at`
/// clause keeps teams that predate lineup snapshots scoreable.
///
/// Binds `$1` = match week id, `$2` = that week's end date.
pub fn scored_teams() -> &'static str {
    r#"SELECT ft.id,
              tgl.id AS lineup_id,
              COALESCE(tgl.captain_id, ft.captain_id) AS captain_id
       FROM fantasy_teams ft
       LEFT JOIN team_gameweek_lineups tgl
         ON tgl.team_id = ft.id AND tgl.match_week_id = $1
       WHERE tgl.id IS NOT NULL
          OR ft.created_at::date <= $2"#
}

/// Season points each squad member earned *for a given team*, summed per gameweek
/// so both the assigned role and that week's captaincy apply.
///
/// Binds `$1` = team id. Selects the player columns plus `assigned_position` and
/// `total_points`. The captain multiplier is inert for bench players, since a
/// captain must be a starter.
pub fn squad_season_points(is_bench: bool) -> String {
    format!(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player,
                  p.team_name, p.photo_url, p.price, p.created_at,
                  tp.assigned_position,
                  COALESCE((
                    SELECT SUM({points} * {captain})
                    FROM player_points pp
                    LEFT JOIN team_gameweek_lineups tgl
                      ON tgl.team_id = ft.id AND tgl.match_week_id = pp.match_week_id
                    WHERE pp.player_id = p.id
                  ), 0)::int AS total_points
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           INNER JOIN fantasy_teams ft ON ft.id = tp.team_id
           WHERE tp.team_id = $1 AND tp.is_bench = {is_bench}"#,
        points = week_points(&scoring_position("tp"), "pp"),
        captain = captain_multiplier(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlayerPosition;
    use crate::services::points_engine::PointsEngine;

    /// Stats for one gameweek, in the order `PointsEngine::calculate` takes them.
    struct Stats {
        goals: i32,
        assists: i32,
        clean_sheets: i32,
        saves: i32,
        penalty_saves: i32,
        own_goals: i32,
        penalty_misses: i32,
        regular_fouls: i32,
        serious_fouls: i32,
        minutes_played: i32,
    }

    fn cases() -> Vec<Stats> {
        vec![
            // Blank week.
            Stats { goals: 0, assists: 0, clean_sheets: 0, saves: 0, penalty_saves: 0,
                    own_goals: 0, penalty_misses: 0, regular_fouls: 0, serious_fouls: 0,
                    minutes_played: 0 },
            // Saves that do not divide evenly by 5, plus a penalty save.
            Stats { goals: 1, assists: 0, clean_sheets: 1, saves: 12, penalty_saves: 1,
                    own_goals: 0, penalty_misses: 0, regular_fouls: 0, serious_fouls: 0,
                    minutes_played: 60 },
            // Fewer than 5 saves should round down to zero.
            Stats { goals: 2, assists: 1, clean_sheets: 1, saves: 3, penalty_saves: 0,
                    own_goals: 0, penalty_misses: 0, regular_fouls: 1, serious_fouls: 0,
                    minutes_played: 40 },
            // Short appearance: 1 point, not 2.
            Stats { goals: 1, assists: 2, clean_sheets: 1, saves: 0, penalty_saves: 0,
                    own_goals: 0, penalty_misses: 0, regular_fouls: 0, serious_fouls: 0,
                    minutes_played: 20 },
            // Exactly on the 35-minute boundary.
            Stats { goals: 0, assists: 0, clean_sheets: 0, saves: 0, penalty_saves: 0,
                    own_goals: 0, penalty_misses: 0, regular_fouls: 0, serious_fouls: 0,
                    minutes_played: 35 },
            // Every deduction at once, enough to go negative.
            Stats { goals: 0, assists: 0, clean_sheets: 0, saves: 0, penalty_saves: 0,
                    own_goals: 1, penalty_misses: 1, regular_fouls: 2, serious_fouls: 1,
                    minutes_played: 60 },
        ]
    }

    fn all_positions() -> [(&'static str, PlayerPosition); 4] {
        [
            ("GK", PlayerPosition::Gk),
            ("DEF", PlayerPosition::Def),
            ("MID", PlayerPosition::Mid),
            ("FWD", PlayerPosition::Fwd),
        ]
    }

    async fn pool() -> Option<sqlx::PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        sqlx::PgPool::connect(&url).await.ok()
    }

    /// The SQL and the Rust engine must never disagree. Two implementations of the
    /// same rules will drift otherwise, and the stored per-gameweek totals come
    /// from the Rust side while every team-scoped query uses the SQL side.
    #[tokio::test]
    async fn sql_matches_rust_engine() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };

        for (pos_text, position) in all_positions() {
            for stats in cases() {
                let sql = format!(
                    "SELECT {expr} FROM (VALUES ($1::int, $2::int, $3::int, $4::int, $5::int, \
                     $6::int, $7::int, $8::int, $9::int, $10::int)) AS pp(goals, assists, \
                     clean_sheets, saves, penalty_saves, own_goals, penalty_misses, \
                     regular_fouls, serious_fouls, minutes_played)",
                    expr = week_points(&format!("'{pos_text}'"), "pp"),
                );

                let from_sql: i32 = sqlx::query_scalar(&sql)
                    .bind(stats.goals)
                    .bind(stats.assists)
                    .bind(stats.clean_sheets)
                    .bind(stats.saves)
                    .bind(stats.penalty_saves)
                    .bind(stats.own_goals)
                    .bind(stats.penalty_misses)
                    .bind(stats.regular_fouls)
                    .bind(stats.serious_fouls)
                    .bind(stats.minutes_played)
                    .fetch_one(&pool)
                    .await
                    .expect("points SQL should execute");

                let from_rust = PointsEngine::calculate(
                    &position,
                    stats.goals,
                    stats.assists,
                    stats.clean_sheets,
                    stats.saves,
                    stats.penalty_saves,
                    stats.own_goals,
                    stats.penalty_misses,
                    stats.regular_fouls,
                    stats.serious_fouls,
                    stats.minutes_played,
                );

                assert_eq!(
                    from_sql, from_rust,
                    "{pos_text} disagreed: SQL gave {from_sql}, engine gave {from_rust} \
                     for {}g {}a {}cs {}sv {}ps {}og {}pm {}rf {}sf {}min",
                    stats.goals, stats.assists, stats.clean_sheets, stats.saves,
                    stats.penalty_saves, stats.own_goals, stats.penalty_misses,
                    stats.regular_fouls, stats.serious_fouls, stats.minutes_played,
                );
            }
        }
    }

    /// A manager's per-player breakdown has to add up to the gameweek total we
    /// store for them, otherwise the numbers on screen cannot be reconciled with
    /// their score. Builds an isolated squad that deliberately plays a forward at
    /// the back, so the assigned-position path is exercised, then rolls back.
    #[tokio::test]
    async fn breakdown_reconciles_with_team_total() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        let user_id: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO users (username, email, password_hash, full_name)
             VALUES ('recon_probe', 'recon_probe@example.test', 'x', 'Recon Probe')
             RETURNING id",
        )
        .fetch_one(&mut *tx)
        .await
        .expect("insert user");

        let team_id: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO fantasy_teams (user_id, name) VALUES ($1, 'Recon FC') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .expect("insert team");

        // 1 GK plus 5 forwards, so the outfield slots can be assigned freely.
        let gk: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM players WHERE position = 'GK' LIMIT 1")
                .fetch_one(&mut *tx)
                .await
                .expect("a GK must exist");
        let outfield: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT id FROM players WHERE position = 'FWD' ORDER BY name LIMIT 5",
        )
        .fetch_all(&mut *tx)
        .await
        .expect("five FWDs must exist");
        assert_eq!(outfield.len(), 5, "seed data needs at least five forwards");

        // A forward played at DEF earns defender rates; if the breakdown and the
        // total disagreed on which position to use, this is where it would show.
        let starters = [
            (gk, "GK"),
            (outfield[0], "DEF"),
            (outfield[1], "DEF"),
            (outfield[2], "MID"),
            (outfield[3], "MID"),
            (outfield[4], "FWD"),
        ];
        for (player_id, pos) in starters {
            sqlx::query(
                "INSERT INTO team_players (team_id, player_id, is_bench, assigned_position)
                 VALUES ($1, $2, false, $3::player_position)",
            )
            .bind(team_id)
            .bind(player_id)
            .bind(pos)
            .execute(&mut *tx)
            .await
            .expect("insert starter");
        }

        let captain = outfield[0];
        sqlx::query("UPDATE fantasy_teams SET captain_id = $1 WHERE id = $2")
            .bind(captain)
            .bind(team_id)
            .execute(&mut *tx)
            .await
            .expect("set captain");

        let week_id: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM match_weeks ORDER BY week_number LIMIT 1")
                .fetch_one(&mut *tx)
                .await
                .expect("a match week must exist");

        // Wipe any pre-existing points for these players so the season sum covers
        // exactly the one week we control.
        let squad: Vec<uuid::Uuid> = starters.iter().map(|(id, _)| *id).collect();
        sqlx::query("DELETE FROM player_points WHERE player_id = ANY($1)")
            .bind(&squad)
            .execute(&mut *tx)
            .await
            .expect("clear points");

        for (i, player_id) in squad.iter().enumerate() {
            sqlx::query(
                "INSERT INTO player_points (player_id, match_week_id, goals, assists,
                     clean_sheets, saves, minutes_played, regular_fouls, total_points)
                 VALUES ($1, $2, $3, 1, 1, 7, 60, 1, 0)",
            )
            .bind(player_id)
            .bind(week_id)
            .bind(i as i32 % 3)
            .execute(&mut *tx)
            .await
            .expect("insert points");
        }

        // Recompute the stored total exactly as `submit_week_stats` does.
        let starter_base: i64 =
            sqlx::query_scalar(&squad_half_total(Source::LiveSquad, false))
                .bind(team_id)
                .bind(week_id)
                .fetch_one(&mut *tx)
                .await
                .expect("starter base");
        let captain_points: i32 = sqlx::query_scalar(&single_starter_total(Source::LiveSquad))
            .bind(team_id)
            .bind(week_id)
            .bind(captain)
            .fetch_optional(&mut *tx)
            .await
            .expect("captain points")
            .unwrap_or(0);
        let gross = starter_base + captain_points as i64;

        // Sum what each starter is shown as having contributed.
        let breakdown: i64 = sqlx::query_scalar(&format!(
            "SELECT COALESCE(SUM(total_points), 0)::bigint FROM ({}) AS s",
            squad_season_points(false)
        ))
        .bind(team_id)
        .fetch_one(&mut *tx)
        .await
        .expect("breakdown");

        assert!(captain_points > 0, "captain should have scored something");
        assert_eq!(
            breakdown, gross,
            "per-player breakdown ({breakdown}) must equal the stored gameweek total \
             ({gross}); starter base {starter_base}, captain bonus {captain_points}"
        );

        tx.rollback().await.expect("rollback");
    }

    /// Re-running an old gameweek must not hand points to managers who joined
    /// afterwards. Without the guard they have no snapshot for that week, fall back
    /// to the live squad, and get scored against stats they were never present for.
    #[tokio::test]
    async fn scoring_skips_teams_that_joined_after_the_week() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut tx = pool.begin().await.expect("begin");

        let (week_id, end_date): (uuid::Uuid, chrono::NaiveDate) =
            sqlx::query_as("SELECT id, end_date FROM match_weeks ORDER BY week_number LIMIT 1")
                .fetch_one(&mut *tx)
                .await
                .expect("a match week must exist");

        // `created_at` is what separates a late joiner from a team that predates
        // lineup snapshots, so each fixture sets it explicitly.
        async fn make_team(
            tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            tag: &str,
            created_at: chrono::NaiveDate,
        ) -> uuid::Uuid {
            let user_id: uuid::Uuid = sqlx::query_scalar(
                "INSERT INTO users (username, email, password_hash, full_name)
                 VALUES ($1, $2, 'x', 'Guard Probe') RETURNING id",
            )
            .bind(format!("guard_{tag}"))
            .bind(format!("guard_{tag}@example.test"))
            .fetch_one(&mut **tx)
            .await
            .expect("insert user");

            sqlx::query_scalar(
                "INSERT INTO fantasy_teams (user_id, name, created_at)
                 VALUES ($1, $2, $3::date) RETURNING id",
            )
            .bind(user_id)
            .bind(format!("Guard {tag}"))
            .bind(created_at)
            .fetch_one(&mut **tx)
            .await
            .expect("insert team")
        }

        let before = end_date - chrono::Duration::days(7);
        let after = end_date + chrono::Duration::days(7);

        let snapshotted = make_team(&mut tx, "snapshotted", after).await;
        let legacy = make_team(&mut tx, "legacy", before).await;
        let late_joiner = make_team(&mut tx, "late", after).await;

        // The snapshotted team joined late too, but played the week, so its snapshot
        // must override the date test.
        sqlx::query(
            "INSERT INTO team_gameweek_lineups (team_id, match_week_id) VALUES ($1, $2)",
        )
        .bind(snapshotted)
        .bind(week_id)
        .execute(&mut *tx)
        .await
        .expect("insert snapshot");

        let scored: Vec<uuid::Uuid> = sqlx::query_scalar(&format!(
            "SELECT id FROM ({}) AS t",
            scored_teams()
        ))
        .bind(week_id)
        .bind(end_date)
        .fetch_all(&mut *tx)
        .await
        .expect("scored teams");

        assert!(
            scored.contains(&snapshotted),
            "a team with a snapshot for the week must be scored"
        );
        assert!(
            scored.contains(&legacy),
            "a team that predates snapshots but existed during the week must be scored"
        );
        assert!(
            !scored.contains(&late_joiner),
            "a team created after the week ended, with no snapshot, must not be scored"
        );

        tx.rollback().await.expect("rollback");
    }

    /// Every composed query must be valid SQL with the expected parameter types.
    /// These are built with `format!` at runtime, so nothing else would catch a
    /// typo until the query ran in production.
    #[tokio::test]
    async fn composed_queries_are_valid_sql() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };

        let id = uuid::Uuid::nil();

        for source in [Source::Snapshot, Source::LiveSquad] {
            for is_bench in [false, true] {
                let sql = squad_half_total(source, is_bench);
                sqlx::query_scalar::<_, i64>(&sql)
                    .bind(id)
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or_else(|e| panic!("squad_half_total({source:?}, {is_bench}): {e}"));
            }

            let sql = single_starter_total(source);
            sqlx::query_scalar::<_, i32>(&sql)
                .bind(id)
                .bind(id)
                .bind(id)
                .fetch_optional(&pool)
                .await
                .unwrap_or_else(|e| panic!("single_starter_total({source:?}): {e}"));
        }

        for is_bench in [false, true] {
            let sql = squad_season_points(is_bench);
            sqlx::query(&sql)
                .bind(id)
                .fetch_all(&pool)
                .await
                .unwrap_or_else(|e| panic!("squad_season_points({is_bench}): {e}"));
        }
    }
}
