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
/// `started` is a predicate for "this player was a starter that week": the
/// gameweek engine only ever pays the bonus to a starter (see
/// [`single_starter_total`]), so a player who was benched that week gets 1x even
/// if they are the captain of record. `week` is the SQL expression for the
/// gameweek. Expects `tgl`, `ft` and `p` in scope.
fn captain_multiplier(started: &str, week: &str) -> String {
    format!(
        r#"CASE
         WHEN ({started}) AND COALESCE(tgl.captain_id, ft.captain_id) = p.id THEN
           CASE WHEN EXISTS (
             SELECT 1 FROM team_chips tc
             WHERE tc.team_id = ft.id
               AND tc.match_week_id = {week}
               AND tc.chip_type = 'triple_captain'
           ) THEN 3 ELSE 2 END
         ELSE 1
       END"#
    )
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

/// Predicate for "the gameweek engine scores this team for this week".
///
/// A team qualifies if it has a lineup snapshot for that week, or if it already
/// existed when the week ended. Without the second test, re-running an old
/// gameweek would fall back to the live squad for managers who joined later and
/// retroactively award them points for a week they never played. The `created_at`
/// clause keeps teams that predate lineup snapshots scoreable.
///
/// `end_date` is the SQL expression for that week's `end_date`; `tgl` is the
/// team's snapshot row for the week (NULL when there is none) and `ft` the team.
/// Every query that decides whether a week counts for a team must use this, so
/// the per-gameweek engine and the squad display cannot drift apart on it.
fn week_is_scored(end_date: &str) -> String {
    format!("(tgl.id IS NOT NULL OR ft.created_at::date <= {end_date})")
}

/// Teams eligible to be scored for a gameweek.
///
/// Binds `$1` = match week id, `$2` = that week's end date.
pub fn scored_teams() -> String {
    format!(
        r#"SELECT ft.id,
                  tgl.id AS lineup_id,
                  COALESCE(tgl.captain_id, ft.captain_id) AS captain_id
           FROM fantasy_teams ft
           LEFT JOIN team_gameweek_lineups tgl
             ON tgl.team_id = ft.id AND tgl.match_week_id = $1
           WHERE {eligible}"#,
        eligible = week_is_scored("$2"),
    )
}

/// Whether a squad member sat on the bench in a given week, resolved the way the
/// gameweek engine resolves it: from that week's snapshot when one exists, and
/// only otherwise from the live squad row.
///
/// Expects `tgl`/`tglp` (that week's snapshot) and `tp` (the live squad row).
fn benched_that_week() -> &'static str {
    "(CASE WHEN tgl.id IS NOT NULL THEN tglp.is_bench ELSE tp.is_bench END)"
}

/// The position a squad member is scored as in a given week, resolved from that
/// week's snapshot when one exists and only otherwise from the live squad row.
///
/// Note this cannot be `COALESCE(tglp.assigned_position, tp.assigned_position,
/// ...)`: a bench player's snapshot row carries a NULL `assigned_position`, and
/// falling through to today's row would score a past week off today's formation.
fn position_that_week() -> String {
    "COALESCE(CASE WHEN tgl.id IS NOT NULL THEN tglp.assigned_position \
     ELSE tp.assigned_position END, p.position)::text"
        .to_string()
}

/// Whether the team played Bench Boost in a given week, so bench points counted.
/// `week` is the SQL expression for the gameweek; expects `ft` in scope.
fn bench_boost_that_week(week: &str) -> String {
    format!(
        r#"EXISTS (
         SELECT 1 FROM team_chips tc
         WHERE tc.team_id = ft.id
           AND tc.match_week_id = {week}
           AND tc.chip_type = 'bench_boost'
       )"#
    )
}

/// FROM/WHERE for the weeks that count towards one squad member's season total
/// *for one team*, ending in a `WHERE` so callers may append further conditions.
///
/// This is the display-side mirror of the per-gameweek engine, and the three
/// conditions are what keep the two in step:
///
/// 1. the week must be one the engine actually scored for this team;
/// 2. once a week has a snapshot, the player only counts if that snapshot lists
///    them — points earned before a transfer-in belong to the selling manager;
/// 3. a bench week only counts under Bench Boost, exactly as `bench_bonus` does.
///
/// Expects `p` (players), `tp` (team_players) and `ft` (fantasy_teams) in scope,
/// and brings `w`, `pp`, `tgl` and `tglp` into scope for the summand.
fn season_weeks_from() -> String {
    format!(
        r#"FROM match_weeks w
           JOIN player_points pp
             ON pp.player_id = p.id AND pp.match_week_id = w.id
           LEFT JOIN team_gameweek_lineups tgl
             ON tgl.team_id = ft.id AND tgl.match_week_id = w.id
           LEFT JOIN team_gameweek_lineup_players tglp
             ON tglp.team_gameweek_lineup_id = tgl.id AND tglp.player_id = p.id
           WHERE {eligible}
             AND (tgl.id IS NULL OR tglp.id IS NOT NULL)
             AND (NOT {benched} OR {bench_boost})"#,
        eligible = week_is_scored("w.end_date"),
        benched = benched_that_week(),
        bench_boost = bench_boost_that_week("w.id"),
    )
}

/// One week's contribution to a squad member's displayed season total, scored on
/// the role they were played in *that* week and multiplied by that week's
/// captaincy. Pairs with [`season_weeks_from`].
fn season_points_summand() -> String {
    format!(
        "{points} * {captain}",
        points = week_points(&position_that_week(), "pp"),
        captain = captain_multiplier(&format!("NOT {}", benched_that_week()), "w.id"),
    )
}

/// Season points each squad member earned *for a given team*, summed per gameweek
/// so both the role they were played in that week and that week's captaincy apply.
///
/// Binds `$1` = team id. Selects the player columns plus the *current*
/// `assigned_position` — the squad page shows today's formation — while
/// `total_points` is historical and is resolved per week from that week's
/// snapshot, so re-arranging the squad today cannot rewrite what past gameweeks
/// paid.
pub fn squad_season_points(is_bench: bool) -> String {
    format!(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.is_top_player,
                  p.team_name, p.photo_url, p.price, p.created_at,
                  tp.assigned_position,
                  COALESCE((
                    SELECT SUM({summand})
                    {weeks}
                  ), 0)::int AS total_points
           FROM players p
           INNER JOIN team_players tp ON p.id = tp.player_id
           INNER JOIN fantasy_teams ft ON ft.id = tp.team_id
           WHERE tp.team_id = $1 AND tp.is_bench = {is_bench}"#,
        summand = season_points_summand(),
        weeks = season_weeks_from(),
    )
}

/// Every `player_points` column, with only `keep` carrying its real value and
/// the rest zeroed.
///
/// Feeding this to [`week_points`] isolates one stat's contribution without
/// naming a single rate: the same expression that scores a full week scores a
/// week in which the player did nothing else. Because `week_points` is a sum of
/// independent terms — the only awkward ones, integer save division and the
/// minutes bucket, each sit in a term of their own — the isolated components add
/// up to the whole exactly. `breakdown_columns_add_up` pins that.
fn isolated_stat(keep: &[&str]) -> String {
    const COLUMNS: [&str; 10] = [
        "goals", "assists", "clean_sheets", "saves", "penalty_saves", "own_goals",
        "penalty_misses", "regular_fouls", "serious_fouls", "minutes_played",
    ];
    COLUMNS
        .iter()
        .map(|column| {
            if keep.contains(column) {
                format!("COALESCE(pp.{column}, 0) AS {column}")
            } else {
                format!("0 AS {column}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The stat groups a gameweek breakdown is shown in, each with the columns it
/// covers. Grouped the way a manager reads a scoreline, not the way the table
/// stores it: the four deductions are one "penalties" line.
const BREAKDOWN_PARTS: [(&str, &[&str]); 6] = [
    ("goal_points", &["goals"]),
    ("assist_points", &["assists"]),
    ("clean_sheet_points", &["clean_sheets"]),
    ("save_points", &["saves", "penalty_saves"]),
    ("minutes_points", &["minutes_played"]),
    (
        "deduction_points",
        &["own_goals", "penalty_misses", "regular_fouls", "serious_fouls"],
    ),
];

/// One team's gameweek, player by player, read from that week's lineup snapshot.
///
/// Binds `$1` = team id, `$2` = match week id. Returns nothing at all when the
/// team has no snapshot for that week, which is the honest answer for gameweeks
/// that predate snapshots — callers must not fall back to the live squad, since
/// scoring a settled week from today's squad is what made past weeks mutate.
///
/// Every number comes from [`week_points`]: `base_points` is the whole
/// expression, and each component is the same expression over a row where only
/// that stat survives. `counted` says whether the player's points reached the
/// team total, which is false for a bench player in a week without Bench Boost.
pub fn gameweek_breakdown() -> String {
    let played_as = scoring_position("tglp");
    let started = "NOT tglp.is_bench";
    let counted = format!(
        "(NOT tglp.is_bench OR {})",
        bench_boost_that_week("tgl.match_week_id")
    );
    let multiplier = captain_multiplier(started, "tgl.match_week_id");
    let base = week_points(&played_as, "pp");

    let components = BREAKDOWN_PARTS
        .iter()
        .map(|(name, _)| format!("{} AS {name}", week_points(&played_as, name)))
        .collect::<Vec<_>>()
        .join(",\n                  ");

    let laterals = BREAKDOWN_PARTS
        .iter()
        .map(|(name, keep)| {
            format!(
                "CROSS JOIN LATERAL (SELECT {}) AS {name}",
                isolated_stat(keep)
            )
        })
        .collect::<Vec<_>>()
        .join("\n           ");

    format!(
        r#"SELECT p.id, p.name, p.position, p.secondary_position, p.team_name, p.photo_url,
                  {played_as} AS played_as,
                  tglp.is_bench,
                  (COALESCE(tgl.captain_id, ft.captain_id) = p.id) AS is_captain,
                  COALESCE(pp.goals, 0) AS goals,
                  COALESCE(pp.assists, 0) AS assists,
                  COALESCE(pp.clean_sheets, 0) AS clean_sheets,
                  COALESCE(pp.saves, 0) AS saves,
                  COALESCE(pp.penalty_saves, 0) AS penalty_saves,
                  COALESCE(pp.own_goals, 0) AS own_goals,
                  COALESCE(pp.penalty_misses, 0) AS penalty_misses,
                  COALESCE(pp.regular_fouls, 0) AS regular_fouls,
                  COALESCE(pp.serious_fouls, 0) AS serious_fouls,
                  COALESCE(pp.minutes_played, 0) AS minutes_played,
                  {components},
                  {base} AS base_points,
                  {multiplier} AS multiplier,
                  {counted} AS counted,
                  CASE WHEN {counted} THEN {base} * {multiplier} ELSE 0 END AS total_points
           FROM team_gameweek_lineups tgl
           JOIN team_gameweek_lineup_players tglp
             ON tglp.team_gameweek_lineup_id = tgl.id
           JOIN players p ON p.id = tglp.player_id
           JOIN fantasy_teams ft ON ft.id = tgl.team_id
           LEFT JOIN player_points pp
             ON pp.player_id = p.id AND pp.match_week_id = tgl.match_week_id
           {laterals}
           WHERE tgl.team_id = $1 AND tgl.match_week_id = $2
           ORDER BY tglp.is_bench,
                    CASE {played_as}
                      WHEN 'GK' THEN 0 WHEN 'DEF' THEN 1 WHEN 'MID' THEN 2 ELSE 3
                    END,
                    p.name"#
    )
}

/// Per-(team, gameweek) reconciliation of the three numbers that must agree:
/// what is stored in `team_gameweek_points`, what a fresh run of the per-gameweek
/// engine produces from the snapshot path, and what the squad endpoints display.
///
/// Binds `$1` = a team id to restrict to, or NULL for every team. Covers every
/// gameweek that has either player stats or a stored team score. Returns one row
/// per (team, week) — callers filter to the disagreements.
///
/// `recomputed_gross` replays [`squad_half_total`], [`single_starter_total`] and
/// the chip handling in `admin::submit_week_stats` as set arithmetic;
/// `displayed_*` replays [`squad_season_points`] restricted to a single week, so
/// this report tracks whatever those functions currently do.
pub fn reconciliation() -> String {
    format!(
        r#"WITH weeks AS (
             SELECT w.id AS week_id, w.week_number, w.end_date
             FROM match_weeks w
             WHERE EXISTS (SELECT 1 FROM player_points pp WHERE pp.match_week_id = w.id)
                OR EXISTS (SELECT 1 FROM team_gameweek_points g WHERE g.match_week_id = w.id)
           ),
           -- The squad the engine scores each (team, week) from: the snapshot when
           -- one exists, the live squad otherwise.
           scored_squad AS (
             SELECT tgl.team_id, tgl.match_week_id, tglp.player_id, tglp.is_bench,
                    {snap_pos} AS pos,
                    EXISTS (SELECT 1 FROM team_players tpx
                            WHERE tpx.team_id = tgl.team_id
                              AND tpx.player_id = tglp.player_id) AS still_in_squad
             FROM team_gameweek_lineups tgl
             JOIN team_gameweek_lineup_players tglp
               ON tglp.team_gameweek_lineup_id = tgl.id
             JOIN players p ON p.id = tglp.player_id
             UNION ALL
             SELECT ft.id, w.week_id, tp.player_id, tp.is_bench, {live_pos}, true
             FROM fantasy_teams ft
             CROSS JOIN weeks w
             JOIN team_players tp ON tp.team_id = ft.id
             JOIN players p ON p.id = tp.player_id
             WHERE NOT EXISTS (
               SELECT 1 FROM team_gameweek_lineups tgl
               WHERE tgl.team_id = ft.id AND tgl.match_week_id = w.week_id
             )
           ),
           pairs AS (
             SELECT ft.id AS team_id, ft.name AS team_name, u.username AS manager,
                    w.week_id, w.week_number,
                    COALESCE(tgl.captain_id, ft.captain_id) AS captain_id,
                    tgl.id IS NOT NULL AS has_snapshot,
                    {eligible} AS engine_scores,
                    EXISTS (SELECT 1 FROM team_chips tc
                            WHERE tc.team_id = ft.id AND tc.match_week_id = w.week_id
                              AND tc.chip_type = 'triple_captain') AS triple_captain,
                    EXISTS (SELECT 1 FROM team_chips tc
                            WHERE tc.team_id = ft.id AND tc.match_week_id = w.week_id
                              AND tc.chip_type = 'bench_boost') AS bench_boost
             FROM fantasy_teams ft
             JOIN users u ON u.id = ft.user_id
             CROSS JOIN weeks w
             LEFT JOIN team_gameweek_lineups tgl
               ON tgl.team_id = ft.id AND tgl.match_week_id = w.week_id
             WHERE $1::uuid IS NULL OR ft.id = $1
           )
           SELECT pr.team_id, pr.team_name, pr.manager, pr.week_number,
                  pr.engine_scores, pr.has_snapshot, pr.triple_captain, pr.bench_boost,
                  g.team_id IS NOT NULL AS has_stored_row,
                  COALESCE(g.gross_points, 0)::bigint AS stored_gross,
                  COALESCE(g.total_points, 0)::bigint AS stored_total,
                  COALESCE(g.transfer_points_hit, 0)::bigint AS stored_hit,
                  CASE WHEN pr.engine_scores THEN
                    rc.starter_base
                    + CASE WHEN pr.triple_captain THEN rc.captain_points * 2
                           ELSE rc.captain_points END
                    + CASE WHEN pr.bench_boost THEN rc.bench_base ELSE 0 END
                  ELSE 0 END::bigint AS recomputed_gross,
                  CASE WHEN pr.engine_scores THEN
                    rc.starter_base_kept
                    + CASE WHEN pr.triple_captain THEN rc.captain_points_kept * 2
                           ELSE rc.captain_points_kept END
                    + CASE WHEN pr.bench_boost THEN rc.bench_base_kept ELSE 0 END
                  ELSE 0 END::bigint AS recomputed_current_squad,
                  rc.starter_base::bigint, rc.bench_base::bigint,
                  rc.captain_points::bigint,
                  dp.displayed_starters::bigint,
                  dp.displayed_bench::bigint,
                  (dp.displayed_starters + dp.displayed_bench)::bigint AS displayed_total
           FROM pairs pr
           LEFT JOIN team_gameweek_points g
             ON g.team_id = pr.team_id AND g.match_week_id = pr.week_id
           CROSS JOIN LATERAL (
             SELECT
               COALESCE(SUM({sq_points}) FILTER (WHERE NOT es.is_bench), 0) AS starter_base,
               COALESCE(SUM({sq_points}) FILTER (WHERE es.is_bench), 0) AS bench_base,
               COALESCE(SUM({sq_points}) FILTER (
                 WHERE NOT es.is_bench AND es.player_id = pr.captain_id), 0) AS captain_points,
               COALESCE(SUM({sq_points}) FILTER (
                 WHERE NOT es.is_bench AND es.still_in_squad), 0) AS starter_base_kept,
               COALESCE(SUM({sq_points}) FILTER (
                 WHERE es.is_bench AND es.still_in_squad), 0) AS bench_base_kept,
               COALESCE(SUM({sq_points}) FILTER (
                 WHERE NOT es.is_bench AND es.still_in_squad
                   AND es.player_id = pr.captain_id), 0) AS captain_points_kept
             FROM scored_squad es
             LEFT JOIN player_points pp
               ON pp.player_id = es.player_id AND pp.match_week_id = pr.week_id
             WHERE es.team_id = pr.team_id AND es.match_week_id = pr.week_id
           ) rc
           CROSS JOIN LATERAL (
             SELECT
               COALESCE(SUM(v.pts) FILTER (WHERE NOT tp.is_bench), 0) AS displayed_starters,
               COALESCE(SUM(v.pts) FILTER (WHERE tp.is_bench), 0) AS displayed_bench
             FROM team_players tp
             JOIN players p ON p.id = tp.player_id
             JOIN fantasy_teams ft ON ft.id = tp.team_id
             CROSS JOIN LATERAL (
               SELECT COALESCE(SUM({summand}), 0) AS pts
               {weeks} AND w.id = pr.week_id
             ) v
             WHERE tp.team_id = pr.team_id
           ) dp
           ORDER BY pr.team_name, pr.week_number"#,
        snap_pos = scoring_position("tglp"),
        live_pos = scoring_position("tp"),
        eligible = week_is_scored("w.end_date"),
        sq_points = week_points("es.pos", "pp"),
        summand = season_points_summand(),
        weeks = season_weeks_from(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::scoring::LineupFreeze;






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

    /// A manager's per-player breakdown has to add up to the gameweek total
    /// stored for them, otherwise the numbers on screen cannot be reconciled
    /// with their score.
    ///
    /// Covers the legacy path — a week with no lineup snapshot, which both the
    /// engine and the squad page fall back to scoring from the live squad — and
    /// deliberately plays forwards at the back so the assigned-position path is
    /// exercised.
    #[tokio::test]
    async fn breakdown_reconciles_with_team_total() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "breakdown", 9700).await;

        // 1 GK plus 5 forwards, so the outfield slots can be assigned freely.
        let gk = w.players("GK", 1).await[0];
        let outfield = w.players("FWD", 5).await;
        let starters = [
            (gk, "GK"),
            (outfield[0], "DEF"),
            (outfield[1], "DEF"),
            (outfield[2], "MID"),
            (outfield[3], "MID"),
            (outfield[4], "FWD"),
        ];
        for (player_id, pos) in starters {
            w.sign(player_id, false, Some(pos)).await;
        }

        let captain = outfield[0];
        w.set_captain(Some(captain)).await;

        // No snapshot for this week: the live-squad fallback.
        let week = w.week(0).await;
        for (i, (player_id, _)) in starters.iter().enumerate() {
            w.stats(
                *player_id,
                week,
                Line {
                    goals: i as i32 % 3,
                    assists: 1,
                    clean_sheets: 1,
                    saves: 7,
                    minutes: 60,
                    regular_fouls: 1,
                    ..Line::default()
                },
            )
            .await;
        }

        let score = w.score(week).await;
        assert!(score.captain_points > 0, "captain should have scored something");

        let breakdown = w.displayed_squad_total(false).await;
        assert_eq!(
            breakdown, score.gross_points as i64,
            "per-player breakdown ({breakdown}) must equal the stored gameweek total \
             ({}); starter base {}, captain bonus {}",
            score.gross_points, score.starter_base, score.captain_bonus,
        );

        assert_reconciles(&w.reconcile().await);
        w.close().await;
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

    // ---------------------------------------------------------------------------
    // Reconciliation harness
    // ---------------------------------------------------------------------------

    /// One row of [`reconciliation`]: what a team's gameweek is worth, three ways.
    #[derive(Debug, Clone, sqlx::FromRow)]
    struct ReconRow {
        team_name: String,
        manager: String,
        week_number: i32,
        engine_scores: bool,
        has_snapshot: bool,
        triple_captain: bool,
        bench_boost: bool,
        has_stored_row: bool,
        /// What `team_gameweek_points` holds.
        stored_gross: i64,
        stored_total: i64,
        stored_hit: i64,
        /// What the per-gameweek engine produces today from the snapshot path.
        recomputed_gross: i64,
        /// The part of `recomputed_gross` earned by players still in the squad —
        /// the most the squad endpoints could truthfully display for that week.
        recomputed_current_squad: i64,
        starter_base: i64,
        bench_base: i64,
        captain_points: i64,
        /// What the squad endpoints actually return, sliced to this week.
        displayed_starters: i64,
        displayed_bench: i64,
        displayed_total: i64,
    }

    impl ReconRow {
        fn label(&self) -> String {
            format!(
                "{} ({}) GW{}{}{}{}",
                self.team_name,
                self.manager,
                self.week_number,
                if self.has_snapshot { "" } else { " [no snapshot]" },
                if self.triple_captain { " [TC]" } else { "" },
                if self.bench_boost { " [BB]" } else { "" },
            )
        }

        /// The stored score must be what the engine produces today. A difference
        /// means the stored week and a re-run disagree.
        fn stored_delta(&self) -> i64 {
            self.stored_gross - self.recomputed_gross
        }

        /// The per-player numbers on screen must add up to the part of the week the
        /// current squad actually earned.
        fn display_delta(&self) -> i64 {
            self.displayed_total - self.recomputed_current_squad
        }

        fn disagrees(&self) -> bool {
            (self.has_stored_row && self.stored_delta() != 0) || self.display_delta() != 0
        }
    }

    async fn reconcile(
        conn: &mut sqlx::PgConnection,
        team_id: Option<uuid::Uuid>,
    ) -> Vec<ReconRow> {
        sqlx::query_as::<_, ReconRow>(&reconciliation())
            .bind(team_id)
            .fetch_all(conn)
            .await
            .expect("reconciliation query")
    }

    fn print_table(rows: &[ReconRow]) {
        println!(
            "{:<22} {:<14} {:>4} {:>8} {:>10} {:>10} {:>7}",
            "team", "manager", "gw", "stored", "recomputed", "displayed", "delta"
        );
        for r in rows {
            println!(
                "{:<22} {:<14} {:>4} {:>8} {:>10} {:>10} {:>7}",
                r.team_name,
                r.manager,
                r.week_number,
                if r.has_stored_row {
                    r.stored_gross.to_string()
                } else {
                    "-".to_string()
                },
                r.recomputed_gross,
                r.displayed_total,
                r.display_delta(),
            );
        }
    }

    fn assert_reconciles(rows: &[ReconRow]) {
        for r in rows {
            if r.has_stored_row {
                assert_eq!(
                    r.stored_gross,
                    r.recomputed_gross,
                    "{}: stored gross {} but a fresh run of the engine gives {} \
                     (starters {}, captain {}, bench {})",
                    r.label(),
                    r.stored_gross,
                    r.recomputed_gross,
                    r.starter_base,
                    r.captain_points,
                    r.bench_base,
                );
            }
            assert_eq!(
                r.displayed_total,
                r.recomputed_current_squad,
                "{}: the squad endpoints show {} for this week ({} starters + {} bench) \
                 but the current squad earned {} of the week's {}",
                r.label(),
                r.displayed_total,
                r.displayed_starters,
                r.displayed_bench,
                r.recomputed_current_squad,
                r.recomputed_gross,
            );
            let _ = (r.stored_total, r.stored_hit, r.engine_scores);
        }
    }

    // ---------------------------------------------------------------------------
    // Fixtures
    // ---------------------------------------------------------------------------

    /// One player's stat line for one week.
    #[derive(Default, Clone, Copy)]
    struct Line {
        goals: i32,
        assists: i32,
        clean_sheets: i32,
        saves: i32,
        penalty_saves: i32,
        own_goals: i32,
        penalty_misses: i32,
        regular_fouls: i32,
        serious_fouls: i32,
        minutes: i32,
    }

    #[derive(Clone, Copy)]
    struct Week {
        id: uuid::Uuid,
        number: i32,
        end_date: chrono::NaiveDate,
    }

    /// A disposable manager, squad and set of gameweeks inside a transaction the
    /// test rolls back.
    ///
    /// Snapshots and scores go through [`crate::services::scoring`], the same code
    /// the admin handler runs, so a test cannot pass by agreeing with a private
    /// re-implementation of the engine.
    struct World<'a> {
        tx: sqlx::Transaction<'a, sqlx::Postgres>,
        team_id: uuid::Uuid,
        base_week: i32,
    }

    /// Fixture gameweeks sit far in the future, and fixture teams are created
    /// just before them.
    ///
    /// That combination is what isolates a fixture from whatever else is in the
    /// database. `scored_teams` only scores a team for weeks that ended after it
    /// was created, so a team created in 2098 is ineligible for every real
    /// gameweek: its squad members' points from those weeks cannot leak into the
    /// fixture's numbers, even though a fixture signs players other teams own.
    /// Anchoring on fixed dates rather than "now" also keeps tests deterministic.
    const WEEK_EPOCH: (i32, u32, u32) = (2099, 1, 5);
    const FIXTURE_TEAM_CREATED: &str = "2098-12-01";

    impl<'a> World<'a> {
        /// `tag` must be unique per test (usernames and team names are unique), and
        /// so must the `base_week` range: `match_weeks.week_number` is unique, and
        /// two uncommitted transactions inserting the same number would block.
        async fn open(pool: &'a sqlx::PgPool, tag: &str, base_week: i32) -> World<'a> {
            let mut tx = pool.begin().await.expect("begin");

            let user_id: uuid::Uuid = sqlx::query_scalar(
                "INSERT INTO users (username, email, password_hash, full_name)
                 VALUES ($1, $2, 'x', 'Fixture Manager') RETURNING id",
            )
            .bind(format!("recon_{tag}"))
            .bind(format!("recon_{tag}@example.test"))
            .fetch_one(&mut *tx)
            .await
            .expect("insert user");

            // Created just before this fixture's own gameweeks and after every
            // real one, so `scored_teams` picks up the former, skips the latter.
            let team_id: uuid::Uuid = sqlx::query_scalar(
                "INSERT INTO fantasy_teams (user_id, name, created_at)
                 VALUES ($1, $2, $3::date) RETURNING id",
            )
            .bind(user_id)
            .bind(format!("Recon {tag}"))
            .bind(FIXTURE_TEAM_CREATED)
            .fetch_one(&mut *tx)
            .await
            .expect("insert team");

            World { tx, team_id, base_week }
        }

        /// Gameweek `offset` of this fixture: a seven-day week starting at the epoch.
        async fn week(&mut self, offset: i32) -> Week {
            let epoch = chrono::NaiveDate::from_ymd_opt(WEEK_EPOCH.0, WEEK_EPOCH.1, WEEK_EPOCH.2)
                .expect("valid epoch");
            let start = epoch + chrono::Duration::days(7 * offset as i64);
            let end = start + chrono::Duration::days(6);
            let number = self.base_week + offset;

            let id: uuid::Uuid = sqlx::query_scalar(
                "INSERT INTO match_weeks (week_number, start_date, end_date, is_active)
                 VALUES ($1, $2, $3, false) RETURNING id",
            )
            .bind(number)
            .bind(start)
            .bind(end)
            .fetch_one(&mut *self.tx)
            .await
            .expect("insert match week");

            Week { id, number, end_date: end }
        }

        /// `n` players whose natural position is `position`.
        async fn players(&mut self, position: &str, n: i64) -> Vec<uuid::Uuid> {
            let ids: Vec<uuid::Uuid> = sqlx::query_scalar(
                "SELECT id FROM players WHERE position = $1::player_position ORDER BY name LIMIT $2",
            )
            .bind(position)
            .bind(n)
            .fetch_all(&mut *self.tx)
            .await
            .expect("select players");
            assert_eq!(ids.len(), n as usize, "seed data needs {n} {position}s");
            ids
        }

        async fn sign(&mut self, player_id: uuid::Uuid, is_bench: bool, played_as: Option<&str>) {
            sqlx::query(
                "INSERT INTO team_players (team_id, player_id, is_bench, assigned_position)
                 VALUES ($1, $2, $3, $4::player_position)",
            )
            .bind(self.team_id)
            .bind(player_id)
            .bind(is_bench)
            .bind(played_as)
            .execute(&mut *self.tx)
            .await
            .expect("insert squad member");
        }

        async fn release(&mut self, player_id: uuid::Uuid) {
            sqlx::query("DELETE FROM team_players WHERE team_id = $1 AND player_id = $2")
                .bind(self.team_id)
                .bind(player_id)
                .execute(&mut *self.tx)
                .await
                .expect("release player");
        }

        /// Move a squad member between the bench and the starting six, the way
        /// Quick Swap does. A bench player carries no assigned position.
        async fn set_bench(&mut self, player_id: uuid::Uuid, is_bench: bool) {
            sqlx::query(
                "UPDATE team_players
                 SET is_bench = $3,
                     assigned_position = CASE WHEN $3 THEN NULL ELSE assigned_position END
                 WHERE team_id = $1 AND player_id = $2",
            )
            .bind(self.team_id)
            .bind(player_id)
            .bind(is_bench)
            .execute(&mut *self.tx)
            .await
            .expect("set bench");
        }

        async fn player_name(&mut self, player_id: uuid::Uuid) -> String {
            sqlx::query_scalar::<_, String>("SELECT name FROM players WHERE id = $1")
                .bind(player_id)
                .fetch_one(&mut *self.tx)
                .await
                .expect("player name")
        }

        /// Whether the week still takes transfers and chips.
        async fn accepts_changes(&mut self, week: Week) -> bool {
            crate::services::scoring::week_accepts_changes(&mut self.tx, week.id)
                .await
                .expect("week accepts changes")
        }

        /// Re-assign a squad member today, the way the squad page does.
        async fn reassign(&mut self, player_id: uuid::Uuid, played_as: &str) {
            sqlx::query(
                "UPDATE team_players SET assigned_position = $3::player_position
                 WHERE team_id = $1 AND player_id = $2",
            )
            .bind(self.team_id)
            .bind(player_id)
            .bind(played_as)
            .execute(&mut *self.tx)
            .await
            .expect("reassign");
        }

        async fn set_captain(&mut self, player_id: Option<uuid::Uuid>) {
            sqlx::query("UPDATE fantasy_teams SET captain_id = $1 WHERE id = $2")
                .bind(player_id)
                .bind(self.team_id)
                .execute(&mut *self.tx)
                .await
                .expect("set captain");
        }

        async fn play_chip(&mut self, chip: &str, week: Week) {
            sqlx::query(
                "INSERT INTO team_chips (team_id, chip_type, match_week_id) VALUES ($1, $2, $3)",
            )
            .bind(self.team_id)
            .bind(chip)
            .bind(week.id)
            .execute(&mut *self.tx)
            .await
            .expect("play chip");
        }

        async fn record_transfer(&mut self, week: Week, out_id: uuid::Uuid, in_id: uuid::Uuid) {
            sqlx::query(
                "INSERT INTO transfers (team_id, match_week_id, player_out_id, player_in_id)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(self.team_id)
            .bind(week.id)
            .bind(out_id)
            .bind(in_id)
            .execute(&mut *self.tx)
            .await
            .expect("record transfer");
        }

        /// Freeze this team's lineup for the week, exactly as production does.
        async fn snapshot(&mut self, week: Week) {
            crate::services::scoring::snapshot_team_lineup(&mut self.tx, self.team_id, week.id)
                .await
                .expect("snapshot lineup");
        }

        /// Put the week's deadline on one side of now or the other.
        ///
        /// Every guard compares against `NOW()`, which inside a transaction is
        /// the transaction's start time, so it does not move under the test.
        async fn deadline(&mut self, week: Week, interval: &str) {
            sqlx::query(&format!(
                "UPDATE match_weeks SET lineup_deadline = NOW() + interval '{interval}'
                 WHERE id = $1"
            ))
            .bind(week.id)
            .execute(&mut *self.tx)
            .await
            .expect("set deadline");
        }

        /// Rewrite this team's lineup for the week, exactly as a save does.
        async fn refresh(&mut self, week: Week) -> crate::services::scoring::LineupFreeze {
            crate::services::scoring::refresh_team_lineup(&mut self.tx, self.team_id, week.id)
                .await
                .expect("refresh lineup")
        }

        /// The names frozen into a gameweek for this team, starters first.
        async fn frozen_squad(&mut self, week: Week) -> Vec<String> {
            let team_id = self.team_id;
            sqlx::query_scalar::<_, String>(
                "SELECT p.name FROM team_gameweek_lineup_players lp
                 JOIN team_gameweek_lineups l ON l.id = lp.team_gameweek_lineup_id
                 JOIN players p ON p.id = lp.player_id
                 WHERE l.team_id = $1 AND l.match_week_id = $2
                 ORDER BY lp.is_bench, p.name",
            )
            .bind(team_id)
            .bind(week.id)
            .fetch_all(&mut *self.tx)
            .await
            .expect("frozen squad")
        }

        /// Whether a lineup header exists for this team and week at all.
        async fn has_lineup_row(&mut self, week: Week) -> bool {
            let team_id = self.team_id;
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM team_gameweek_lineups
                                 WHERE team_id = $1 AND match_week_id = $2)",
            )
            .bind(team_id)
            .bind(week.id)
            .fetch_one(&mut *self.tx)
            .await
            .expect("has lineup row")
        }

        /// The captain frozen into a gameweek for this team.
        async fn frozen_captain(&mut self, week: Week) -> Option<uuid::Uuid> {
            let team_id = self.team_id;
            sqlx::query_scalar::<_, Option<uuid::Uuid>>(
                "SELECT captain_id FROM team_gameweek_lineups
                 WHERE team_id = $1 AND match_week_id = $2",
            )
            .bind(team_id)
            .bind(week.id)
            .fetch_optional(&mut *self.tx)
            .await
            .expect("frozen captain")
            .flatten()
        }

        /// `total_points` is deliberately left at 0: it is the player's own
        /// primary-position tally and plays no part in team scoring, which always
        /// recomputes from the raw stats.
        async fn stats(&mut self, player_id: uuid::Uuid, week: Week, line: Line) {
            sqlx::query(
                "INSERT INTO player_points (player_id, match_week_id, goals, assists,
                     clean_sheets, saves, penalty_saves, own_goals, penalty_misses,
                     regular_fouls, serious_fouls, minutes_played, total_points)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 0)",
            )
            .bind(player_id)
            .bind(week.id)
            .bind(line.goals)
            .bind(line.assists)
            .bind(line.clean_sheets)
            .bind(line.saves)
            .bind(line.penalty_saves)
            .bind(line.own_goals)
            .bind(line.penalty_misses)
            .bind(line.regular_fouls)
            .bind(line.serious_fouls)
            .bind(line.minutes)
            .execute(&mut *self.tx)
            .await
            .expect("insert stats");
        }

        /// Make this the league's open gameweek.
    async fn open_week(&mut self, week: Week) {
        sqlx::query("UPDATE match_weeks SET is_active = true WHERE id = $1")
            .bind(week.id)
            .execute(&mut *self.tx)
            .await
            .expect("open week");
    }

    /// Which of this fixture's gameweeks is open, if any.
    async fn open_week_number(&mut self) -> Option<i32> {
        let base = self.base_week;
        sqlx::query_scalar::<_, i32>(
            "SELECT week_number FROM match_weeks
             WHERE is_active AND week_number >= $1 AND week_number < $1 + 100",
        )
        .bind(base)
        .fetch_optional(&mut *self.tx)
        .await
        .expect("open week number")
    }

    /// How many players are frozen into a gameweek for this team.
    async fn frozen_count(&mut self, week: Week) -> i64 {
        let team_id = self.team_id;
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM team_gameweek_lineup_players lp
             JOIN team_gameweek_lineups l ON l.id = lp.team_gameweek_lineup_id
             WHERE l.team_id = $1 AND l.match_week_id = $2",
        )
        .bind(team_id)
        .bind(week.id)
        .fetch_one(&mut *self.tx)
        .await
        .expect("frozen count")
    }

    /// Score and store the week through the production engine.
        async fn score(&mut self, week: Week) -> crate::services::scoring::TeamWeekScore {
            let ctx: Option<(uuid::Uuid, Option<uuid::Uuid>, Option<uuid::Uuid>)> =
                sqlx::query_as(&format!(
                    "SELECT id, lineup_id, captain_id FROM ({}) t WHERE id = $3",
                    scored_teams()
                ))
                .bind(week.id)
                .bind(week.end_date)
                .bind(self.team_id)
                .fetch_optional(&mut *self.tx)
                .await
                .expect("scored teams");

            let (_, lineup_id, captain_id) =
                ctx.expect("fixture team must be eligible for its own gameweek");

            let score = crate::services::scoring::score_team_gameweek(
                &mut self.tx,
                self.team_id,
                lineup_id,
                captain_id,
                week.id,
            )
            .await
            .expect("score week");

            crate::services::scoring::store_team_gameweek_score(
                &mut self.tx,
                self.team_id,
                week.id,
                &score,
            )
            .await
            .expect("store score");

            score
        }

        /// The per-player derivation of a gameweek, as the API serves it.
    async fn breakdown(&mut self, week: Week) -> Vec<crate::models::GameweekPlayerLine> {
        let team_id = self.team_id;
        sqlx::query_as::<_, crate::models::GameweekPlayerLine>(&gameweek_breakdown())
            .bind(team_id)
            .bind(week.id)
            .fetch_all(&mut *self.tx)
            .await
            .expect("gameweek breakdown")
    }

    /// Reconciliation rows for this fixture's gameweeks only.
        async fn reconcile(&mut self) -> Vec<ReconRow> {
            let team_id = self.team_id;
            let base = self.base_week;
            let mut rows = reconcile(&mut self.tx, Some(team_id)).await;
            rows.retain(|r| r.week_number >= base && r.week_number < base + 100);
            rows
        }

        /// What a squad member's season total is shown as on the squad page.
        async fn displayed_total(&mut self, player_id: uuid::Uuid, is_bench: bool) -> i32 {
            let team_id = self.team_id;
            sqlx::query_scalar::<_, i32>(&format!(
                "SELECT total_points FROM ({}) s WHERE id = $2",
                squad_season_points(is_bench)
            ))
            .bind(team_id)
            .bind(player_id)
            .fetch_one(&mut *self.tx)
            .await
            .expect("displayed total")
        }

        /// The sum of the per-player numbers one half of the squad page shows.
    async fn displayed_squad_total(&mut self, is_bench: bool) -> i64 {
        let team_id = self.team_id;
        sqlx::query_scalar::<_, i64>(&format!(
            "SELECT COALESCE(SUM(total_points), 0)::bigint FROM ({}) s",
            squad_season_points(is_bench)
        ))
        .bind(team_id)
        .fetch_one(&mut *self.tx)
        .await
        .expect("displayed squad total")
    }

    async fn close(self) {
            self.tx.rollback().await.expect("rollback");
        }
    }

    /// A full nine-player squad: 1 GK + 5 outfield starters, 1 GK + 2 outfield bench.
    /// Returns (starters, bench) in the order they were signed.
    async fn standard_squad(w: &mut World<'_>) -> (Vec<uuid::Uuid>, Vec<uuid::Uuid>) {
        let gks = w.players("GK", 2).await;
        let defs = w.players("DEF", 3).await;
        let mids = w.players("MID", 2).await;
        let fwds = w.players("FWD", 2).await;

        let starters = vec![gks[0], defs[0], defs[1], mids[0], mids[1], fwds[0]];
        for (id, pos) in [
            (gks[0], "GK"),
            (defs[0], "DEF"),
            (defs[1], "DEF"),
            (mids[0], "MID"),
            (mids[1], "MID"),
            (fwds[0], "FWD"),
        ] {
            w.sign(id, false, Some(pos)).await;
        }

        let bench = vec![gks[1], defs[2], fwds[1]];
        for id in &bench {
            w.sign(*id, true, None).await;
        }

        (starters, bench)
    }

    // ---------------------------------------------------------------------------
    // The report
    // ---------------------------------------------------------------------------

    /// Reconcile every team and every scored gameweek in the database.
    ///
    /// Run it against production data with
    /// `DATABASE_URL=... cargo test reconciliation_report -- --nocapture`.
    #[tokio::test]
    async fn reconciliation_report() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut conn = pool.acquire().await.expect("acquire");

        let rows = reconcile(&mut conn, None).await;
        let disagreeing: Vec<ReconRow> = rows.iter().filter(|r| r.disagrees()).cloned().collect();

        println!("checked {} team/gameweek rows", rows.len());
        if disagreeing.is_empty() {
            println!("no disagreements");
        } else {
            println!("{} disagreeing rows:", disagreeing.len());
            print_table(&disagreeing);
        }

        assert_reconciles(&rows);
    }

    // ---------------------------------------------------------------------------
    // Regression tests, one per confirmed cause
    // ---------------------------------------------------------------------------

    /// A past gameweek must keep paying the rates for the role the player was
    /// actually played in that week. Re-arranging the squad today is a decision
    /// about next week, and must not rewrite what a scored week was worth.
    #[tokio::test]
    async fn reassigning_a_position_today_does_not_rewrite_a_scored_week() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "position", 9100).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        let (gk, played_at_def) = (starters[0], starters[1]);
        // Captain someone uninvolved so the captaincy cannot mask the effect.
        w.set_captain(Some(gk)).await;

        let week = w.week(0).await;
        w.snapshot(week).await;

        // Two goals from a player fielded at the back: 2 x 6 + 2 for the minutes.
        w.stats(played_at_def, week, Line { goals: 2, minutes: 60, ..Line::default() })
            .await;
        let score = w.score(week).await;
        assert_eq!(score.starter_base, 14, "DEF rates: 2 goals x 6 + 2 minutes");

        // The manager moves the same player up front for the week ahead.
        w.reassign(played_at_def, "FWD").await;

        let rows = w.reconcile().await;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].displayed_total, 14,
            "the scored week must still pay DEF rates, not the 10 it would be worth \
             at the position the player was moved to afterwards"
        );
        assert_reconciles(&rows);
        w.close().await;
    }

    /// Points a player scored before a manager signed them belong to whoever owned
    /// them at the time. Buying a player must not buy their history.
    #[tokio::test]
    async fn a_transfer_in_does_not_buy_the_players_history() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "transfer", 9200).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        let leaving = starters[5];
        let arriving = w.players("FWD", 3).await[2];
        w.set_captain(Some(starters[0])).await;

        // Week one: the manager does not own `arriving`, who has a big week elsewhere.
        let week1 = w.week(0).await;
        w.snapshot(week1).await;
        w.stats(arriving, week1, Line { goals: 3, assists: 2, minutes: 90, ..Line::default() })
            .await;
        w.score(week1).await;

        // Week two: the snapshot freezes the pre-transfer squad, then the swap lands.
        let week2 = w.week(1).await;
        w.snapshot(week2).await;
        w.record_transfer(week2, leaving, arriving).await;
        w.release(leaving).await;
        w.sign(arriving, false, Some("FWD")).await;
        w.score(week2).await;

        assert_eq!(
            w.displayed_total(arriving, false).await,
            0,
            "the new signing's 24 points from before the transfer must not appear \
             on the buying manager's squad page"
        );

        let rows = w.reconcile().await;
        assert_eq!(rows.len(), 2);
        assert_reconciles(&rows);
        w.close().await;
    }

    /// Scoring a gameweek closes it, and a closed week reports itself as scored.
    ///
    /// GW3 in production stayed active for seven days after it was scored, which
    /// is how a Triple Captain came to be played against a week whose points were
    /// already stored: the chip counted for nothing and could not be played again.
    #[tokio::test]
    async fn scoring_a_gameweek_closes_it() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "closes", 9950).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let week = w.week(0).await;
        w.snapshot(week).await;
        w.stats(starters[1], week, Line { goals: 1, minutes: 90, ..Line::default() })
            .await;

        assert!(
            !crate::services::scoring::week_already_scored(&mut w.tx, week.id)
                .await
                .expect("scored check"),
            "an unscored week must be open for chips, transfers and lineup changes"
        );

        w.score(week).await;

        assert!(
            crate::services::scoring::week_already_scored(&mut w.tx, week.id)
                .await
                .expect("scored check"),
            "once points are stored the week is closed, and the handlers must \
             refuse chips, transfers and lineup changes aimed at it"
        );

        assert_reconciles(&w.reconcile().await);
        w.close().await;
    }

    /// Scoring the open gameweek rolls the league on: that week closes and the
    /// next opens, with every squad frozen into it.
    ///
    /// The seven days GW3 spent scored-but-still-open in production is what cost
    /// one manager their Triple Captain and let ten players be appended to
    /// lineups that were supposed to be settled.
    #[tokio::test]
    async fn scoring_the_open_gameweek_opens_the_next_one() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "rollover", 9970).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let this_week = w.week(0).await;
        let next_week = w.week(1).await;
        w.open_week(this_week).await;
        w.snapshot(this_week).await;
        w.stats(starters[1], this_week, Line { goals: 1, minutes: 90, ..Line::default() })
            .await;

        assert_eq!(w.open_week_number().await, Some(this_week.number));
        assert_eq!(w.frozen_count(next_week).await, 0, "the next week is not frozen yet");

        w.score(this_week).await;
        let opened = crate::services::scoring::close_week_and_open_next(
            &mut w.tx,
            this_week.id,
            this_week.number,
            true,
        )
        .await
        .expect("roll the league on");

        assert_eq!(opened, Some(next_week.number));
        assert_eq!(
            w.open_week_number().await,
            Some(next_week.number),
            "the scored week must close and the next one open"
        );
        assert_eq!(
            w.frozen_count(next_week).await,
            9,
            "every squad is frozen into the week that just opened, so a manager \
             who never touches their team still has a lineup for it"
        );
        assert!(
            crate::services::scoring::week_already_scored(&mut w.tx, this_week.id)
                .await
                .expect("scored check"),
            "and the week it closed stays closed to chips, transfers and lineups"
        );

        // Re-scoring a settled week to correct it must not wind the league back.
        let again = crate::services::scoring::close_week_and_open_next(
            &mut w.tx,
            this_week.id,
            this_week.number,
            false,
        )
        .await
        .expect("re-score an old week");
        assert_eq!(again, None, "re-scoring an old week moves nothing");
        assert_eq!(w.open_week_number().await, Some(next_week.number));

        assert_reconciles(&w.reconcile().await);
        w.close().await;
    }

    /// The breakdown a manager is shown has to add up in front of them: each
    /// player's columns to their line, and every counted line to the total the
    /// league table shows.
    #[tokio::test]
    async fn breakdown_columns_add_up() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "breakdowncols", 9820).await;

        let (starters, bench) = standard_squad(&mut w).await;
        let captain = starters[3];
        w.set_captain(Some(captain)).await;

        let week = w.week(0).await;
        w.snapshot(week).await;
        w.play_chip("triple_captain", week).await;

        // A spread wide enough that every column has something in it.
        let lines = [
            (starters[0], Line { clean_sheets: 1, saves: 7, penalty_saves: 1, minutes: 60, ..Line::default() }),
            (starters[1], Line { goals: 1, clean_sheets: 1, minutes: 60, ..Line::default() }),
            (starters[2], Line { clean_sheets: 1, regular_fouls: 2, serious_fouls: 1, minutes: 60, ..Line::default() }),
            (starters[3], Line { goals: 2, assists: 1, minutes: 90, ..Line::default() }),
            (starters[4], Line { assists: 2, minutes: 20, ..Line::default() }),
            (starters[5], Line { goals: 1, own_goals: 1, penalty_misses: 1, minutes: 45, ..Line::default() }),
            (bench[0], Line { minutes: 0, ..Line::default() }),
            (bench[1], Line { goals: 1, minutes: 40, ..Line::default() }),
            (bench[2], Line { assists: 1, minutes: 50, ..Line::default() }),
        ];
        for (player, line) in lines {
            w.stats(player, week, line).await;
        }

        let score = w.score(week).await;
        let rows = w.breakdown(week).await;
        assert_eq!(rows.len(), 9, "every squad member gets a line");

        for row in &rows {
            assert_eq!(
                row.goal_points
                    + row.assist_points
                    + row.clean_sheet_points
                    + row.save_points
                    + row.minutes_points
                    + row.deduction_points,
                row.base_points,
                "{}'s columns must add up to their line",
                row.name,
            );
            assert_eq!(
                row.total_points,
                if row.counted { row.base_points * row.multiplier } else { 0 },
                "{}'s total must be their line times their multiplier",
                row.name,
            );
        }

        // No Bench Boost this week, so the bench is shown but does not count.
        for row in rows.iter().filter(|r| r.is_bench) {
            assert!(!row.counted, "{} was benched in a week with no boost", row.name);
            assert_eq!(row.total_points, 0);
        }

        // Triple Captain was played, so the captain is worth three times.
        let skipper = rows.iter().find(|r| r.is_captain).expect("a captain");
        assert_eq!(skipper.multiplier, 3, "Triple Captain was played");

        let shown: i32 = rows.iter().map(|r| r.total_points).sum();
        assert_eq!(
            shown, score.gross_points,
            "the breakdown must add up to the gameweek total the league table shows"
        );

        w.close().await;
    }

    /// Under Bench Boost the bench counts, and the breakdown says so.
    #[tokio::test]
    async fn breakdown_counts_the_bench_under_bench_boost() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "breakdownbb", 9830).await;

        let (starters, bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let week = w.week(0).await;
        w.snapshot(week).await;
        w.play_chip("bench_boost", week).await;
        w.stats(bench[1], week, Line { goals: 1, minutes: 60, ..Line::default() })
            .await;

        let score = w.score(week).await;
        let rows = w.breakdown(week).await;

        assert!(
            rows.iter().filter(|r| r.is_bench).all(|r| r.counted),
            "Bench Boost was played, so every bench line counts"
        );
        let shown: i32 = rows.iter().map(|r| r.total_points).sum();
        assert_eq!(shown, score.gross_points);

        w.close().await;
    }

    /// The regression this whole deadline machinery exists for.
    ///
    /// Gameweek 5 in production was frozen on the Monday it opened, six days
    /// before its Saturday deadline. Ten of thirty-two managers were then
    /// scored on the squad they had held a week earlier: six transfers made on
    /// the Saturday evening, and every re-arrangement of the week, went to a
    /// lineup nothing ever read.
    #[tokio::test]
    async fn a_save_before_the_deadline_reaches_the_week() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "beats_deadline", 9640).await;

        let (starters, bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[5])).await;

        // The week opens and seeds every squad, exactly as production does.
        let week = w.week(0).await;
        w.deadline(week, "1 day").await;
        w.snapshot(week).await;

        // Days pass, and the manager benches a starter for a bench player who
        // goes on to score. Under the old code this never left `team_players`.
        let dropped = starters[3];
        let promoted = bench[1];
        w.set_bench(dropped, true).await;
        w.set_bench(promoted, false).await;
        w.reassign(promoted, "DEF").await;
        assert_eq!(w.refresh(week).await, LineupFreeze::Refreshed);

        w.stats(promoted, week, Line { goals: 1, minutes: 90, ..Line::default() })
            .await;
        w.stats(dropped, week, Line { goals: 2, minutes: 90, ..Line::default() })
            .await;

        let score = w.score(week).await;
        assert_eq!(
            score.starter_base, 8,
            "the promoted player is scored as the defender they were played as: \
             one goal at 6 plus 2 for the minutes. The lineup frozen at the open \
             would instead have paid the dropped midfielder's two goals at 5 plus \
             2, which is 12, and nothing for the player who actually started"
        );

        let frozen = w.frozen_squad(week).await;
        assert_eq!(frozen.len(), 9, "still nine players, not eleven");
        w.close().await;
    }

    /// Past the deadline the week is settled, and a save cannot reach back into
    /// it.
    ///
    /// This is the other half of the rule, and it is not hypothetical: lineups
    /// reopen at Sunday noon while the week just played is usually still active
    /// and unscored. A manager rearranging then is setting up the next week.
    #[tokio::test]
    async fn a_save_after_the_deadline_leaves_the_week_alone() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "past_deadline", 9650).await;

        let (starters, bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let week = w.week(0).await;
        w.deadline(week, "1 day").await;
        w.snapshot(week).await;
        let at_the_deadline = w.frozen_squad(week).await;

        // The deadline passes, then the manager rearranges.
        w.deadline(week, "-1 hour").await;
        w.set_bench(starters[3], true).await;
        w.set_bench(bench[1], false).await;
        w.set_captain(Some(starters[1])).await;

        assert_eq!(
            w.refresh(week).await,
            LineupFreeze::Sealed,
            "a week past its deadline does not take another squad"
        );
        assert_eq!(
            w.frozen_squad(week).await,
            at_the_deadline,
            "the lineup is the one that was in place at the deadline"
        );
        assert_eq!(
            w.frozen_captain(week).await,
            Some(starters[0]),
            "and so is the captain"
        );
        w.close().await;
    }

    /// A refresh replaces the lineup. It must never append to it.
    ///
    /// `ops/2026-08-24_repair_scored_weeks.sql` is the record of what appending
    /// costs: 60 rows accumulated across gameweeks 1-3, lineups grew to as many
    /// as fourteen players, managers were paid for players they had transferred
    /// away, and only the 10 rows that landed after scoring could be safely
    /// removed. The other 50 are still there.
    #[tokio::test]
    async fn a_refresh_replaces_the_lineup_rather_than_adding_to_it() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "replaces", 9660).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let week = w.week(0).await;
        w.deadline(week, "1 day").await;
        w.snapshot(week).await;

        // Three transfers in one week, each one a save.
        let spares = w.players("FWD", 4).await;
        let mut outgoing = starters[5];
        for arriving in [spares[2], spares[3]] {
            w.release(outgoing).await;
            w.sign(arriving, false, Some("FWD")).await;
            assert_eq!(w.refresh(week).await, LineupFreeze::Refreshed);
            outgoing = arriving;
        }

        assert_eq!(
            w.frozen_count(week).await,
            9,
            "nine players after every save, never the union of every squad held"
        );
        let frozen = w.frozen_squad(week).await;
        let departed = w.player_name(starters[5]).await;
        assert!(
            !frozen.contains(&departed),
            "a player transferred away before the deadline must not still be in \
             the lineup: {frozen:?} still holds {departed}"
        );
        w.close().await;
    }

    /// Scoring settles a week for good, whatever its deadline says.
    ///
    /// The deadline and the scored test are separate guards on purpose. An
    /// admin re-activating an old gameweek gives it a fresh deadline, and that
    /// must not reopen a week whose points are already paid.
    #[tokio::test]
    async fn a_scored_week_is_sealed_even_before_its_deadline() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "scored_seal", 9670).await;

        let (starters, bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let week = w.week(0).await;
        w.deadline(week, "1 day").await;
        w.snapshot(week).await;
        w.stats(starters[1], week, Line { goals: 1, minutes: 90, ..Line::default() })
            .await;
        w.score(week).await;

        // Deadline still in the future, but the week is paid for.
        w.set_bench(starters[1], true).await;
        w.set_bench(bench[1], false).await;
        assert_eq!(w.refresh(week).await, LineupFreeze::Sealed);
        w.close().await;
    }

    /// A gameweek with no deadline recorded takes nothing.
    ///
    /// `NOW() < NULL` is NULL, not true, so the guard fails closed. Gameweeks
    /// 7-12 sit in production right now with no deadline, never opened.
    #[tokio::test]
    async fn a_week_with_no_deadline_accepts_nothing() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "no_deadline", 9680).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let week = w.week(0).await;
        w.snapshot(week).await;

        assert_eq!(w.refresh(week).await, LineupFreeze::Sealed);
        assert!(
            !w.accepts_changes(week).await,
            "a week with no deadline must not accept transfers or chips either"
        );
        w.close().await;
    }

    /// A team with no squad gets no lineup row at all.
    ///
    /// A header with nothing under it is worse than no header:
    /// [`week_is_scored`] reads `tgl.id IS NOT NULL` as "this team played the
    /// week", so an empty one has the team scored 0 from an empty snapshot
    /// rather than skipped. Production holds one such row, on gameweek 2, from
    /// a manager who joined mid-week — the old writer created the header before
    /// the squad existed and then never revisited it.
    #[tokio::test]
    async fn a_team_with_no_squad_gets_no_lineup() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "empty_squad", 9700).await;

        let week = w.week(0).await;
        w.deadline(week, "1 day").await;

        assert_eq!(
            w.refresh(week).await,
            LineupFreeze::Sealed,
            "a manager who has not picked a squad yet has no lineup to freeze"
        );
        assert_eq!(w.frozen_count(week).await, 0);
        assert!(
            !w.has_lineup_row(week).await,
            "and no empty header either, which would make them look like they played"
        );

        // Once they pick a squad, the same call writes it.
        standard_squad(&mut w).await;
        assert_eq!(w.refresh(week).await, LineupFreeze::Refreshed);
        assert_eq!(w.frozen_count(week).await, 9);
        w.close().await;
    }

    /// The captain moves with the squad, up to the deadline.
    ///
    /// The armband is read from the snapshot first
    /// (`COALESCE(tgl.captain_id, ft.captain_id)`), so a captain change that
    /// does not reach the snapshot is a doubled score on the wrong player.
    #[tokio::test]
    async fn the_captain_moves_with_the_squad_until_the_deadline() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "armband", 9690).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[1])).await;

        let week = w.week(0).await;
        w.deadline(week, "1 day").await;
        w.snapshot(week).await;

        w.set_captain(Some(starters[5])).await;
        assert_eq!(w.refresh(week).await, LineupFreeze::Refreshed);
        assert_eq!(
            w.frozen_captain(week).await,
            Some(starters[5]),
            "the armband the manager moved before the deadline is the one that pays"
        );
        w.close().await;
    }

    /// A gameweek with no snapshot yields nothing, and must never be answered
    /// with the squad the manager happens to hold today.
    #[tokio::test]
    async fn breakdown_is_empty_without_a_snapshot() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "breakdownnone", 9840).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        // Scored, but never frozen: the shape of a gameweek from before
        // snapshots existed.
        let week = w.week(0).await;
        w.stats(starters[1], week, Line { goals: 2, minutes: 90, ..Line::default() })
            .await;
        let score = w.score(week).await;
        assert!(score.gross_points > 0, "the week did pay points");

        assert!(
            w.breakdown(week).await.is_empty(),
            "with no snapshot the lineup is unknown, and an empty answer is the \
             honest one — the live squad must never stand in for it"
        );

        w.close().await;
    }

    /// The breakdown must agree with the stored score for every gameweek in the
    /// database it is pointed at, not just for fixtures.
    ///
    /// Read-only, so it is safe to run against production:
    /// `DATABASE_URL=... cargo test breakdown_matches_every_stored_score -- --nocapture`
    #[tokio::test]
    async fn breakdown_matches_every_stored_score() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };

        // Only weeks that were both frozen and scored can be checked: a week with
        // no snapshot has no breakdown to compare, by design.
        let subjects: Vec<(uuid::Uuid, uuid::Uuid, String, i32, i32)> = sqlx::query_as(
            "SELECT tgl.team_id, tgl.match_week_id, u.username, w.week_number, g.gross_points
             FROM team_gameweek_lineups tgl
             JOIN match_weeks w ON w.id = tgl.match_week_id
             JOIN fantasy_teams ft ON ft.id = tgl.team_id
             JOIN users u ON u.id = ft.user_id
             JOIN team_gameweek_points g
               ON g.team_id = tgl.team_id AND g.match_week_id = tgl.match_week_id
             ORDER BY w.week_number, u.username",
        )
        .fetch_all(&pool)
        .await
        .expect("subjects");

        let mut mismatches = Vec::new();

        for (team_id, week_id, manager, week_number, gross) in &subjects {
            let rows = sqlx::query_as::<_, crate::models::GameweekPlayerLine>(&gameweek_breakdown())
                .bind(team_id)
                .bind(week_id)
                .fetch_all(&pool)
                .await
                .expect("breakdown");

            for row in &rows {
                assert_eq!(
                    row.goal_points
                        + row.assist_points
                        + row.clean_sheet_points
                        + row.save_points
                        + row.minutes_points
                        + row.deduction_points,
                    row.base_points,
                    "{manager} GW{week_number}: {}'s columns do not add up to their line",
                    row.name,
                );
            }

            let shown: i32 = rows.iter().map(|r| r.total_points).sum();
            if shown != *gross {
                mismatches.push(format!(
                    "{manager} GW{week_number}: breakdown shows {shown}, stored gross is {gross}"
                ));
            }
        }

        println!("checked {} stored gameweeks against their breakdown", subjects.len());
        assert!(
            mismatches.is_empty(),
            "the breakdown must add up to the stored score:\n{}",
            mismatches.join("\n"),
        );
    }

    /// A frozen lineup is written once and never touched again.
    ///
    /// The handlers call `snapshot_team_lineup` before every lineup change and
    /// every transfer, so a week that stays active sees many calls. If any of
    /// them appended the squad members signed since the freeze, re-scoring that
    /// week would pay players the manager did not field in it.
    #[tokio::test]
    async fn a_frozen_lineup_never_gains_a_player() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "frozen", 9900).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let week = w.week(0).await;
        w.snapshot(week).await;

        let frozen: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM team_gameweek_lineup_players lp
             JOIN team_gameweek_lineups l ON l.id = lp.team_gameweek_lineup_id
             WHERE l.team_id = $1 AND l.match_week_id = $2",
        )
        .bind(w.team_id)
        .bind(week.id)
        .fetch_one(&mut *w.tx)
        .await
        .expect("count frozen lineup");
        assert_eq!(frozen, 9, "the freeze captures the nine-player squad");

        // The manager transfers during the week and re-arranges afterwards. Each
        // of those actions re-enters the snapshot path.
        let arriving = w.players("FWD", 3).await[2];
        w.release(starters[5]).await;
        w.sign(arriving, false, Some("FWD")).await;
        w.snapshot(week).await;
        w.snapshot(week).await;

        let after: Vec<(uuid::Uuid, bool)> = sqlx::query_as(
            "SELECT lp.player_id, lp.is_bench FROM team_gameweek_lineup_players lp
             JOIN team_gameweek_lineups l ON l.id = lp.team_gameweek_lineup_id
             WHERE l.team_id = $1 AND l.match_week_id = $2",
        )
        .bind(w.team_id)
        .bind(week.id)
        .fetch_all(&mut *w.tx)
        .await
        .expect("re-read frozen lineup");

        assert_eq!(
            after.len(),
            9,
            "the frozen lineup gained a player it never fielded"
        );
        assert!(
            !after.iter().any(|(id, _)| *id == arriving),
            "a player signed after the freeze must not appear in that week's lineup"
        );
        assert!(
            after.iter().any(|(id, _)| *id == starters[5]),
            "a player sold after the freeze must stay in that week's lineup"
        );

        // And the score the week pays is unchanged by any of it.
        w.stats(arriving, week, Line { goals: 3, minutes: 90, ..Line::default() })
            .await;
        w.stats(starters[1], week, Line { goals: 1, minutes: 90, ..Line::default() })
            .await;
        let score = w.score(week).await;
        assert_eq!(
            score.starter_base, 8,
            "only the fielded squad scores: 1 goal x 6 + 2 minutes, and nothing \
             from the player signed after the freeze"
        );

        assert_reconciles(&w.reconcile().await);
        w.close().await;
    }

    /// The second transfer in a gameweek costs four points. The hit lands on the
    /// team's score, not on any player, so a manager's per-player numbers add up
    /// to their gross and sit four short of their net total — which is the
    /// arithmetic that has to be explained rather than "fixed".
    #[tokio::test]
    async fn a_second_transfer_in_a_week_costs_four_points() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "hit", 9800).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let week = w.week(0).await;
        w.snapshot(week).await;
        w.stats(starters[1], week, Line { goals: 1, minutes: 90, ..Line::default() })
            .await;

        let spares = w.players("FWD", 4).await;
        w.record_transfer(week, starters[5], spares[2]).await;
        w.record_transfer(week, spares[2], spares[3]).await;

        let score = w.score(week).await;
        assert_eq!(score.transfers, 2);
        assert_eq!(score.transfer_points_hit, 4, "one free transfer, then -4");
        assert_eq!(score.total_points, score.gross_points - 4);

        let rows = w.reconcile().await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].stored_hit, 4);
        assert_eq!(rows[0].stored_total, rows[0].stored_gross - 4);
        assert_reconciles(&rows);
        w.close().await;
    }

    /// Bench points count in a Bench Boost week and in no other week. The squad page
    /// has to draw the same line the gameweek engine draws.
    #[tokio::test]
    async fn bench_points_count_only_under_bench_boost() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "bench", 9300).await;

        let (starters, bench) = standard_squad(&mut w).await;
        let benched = bench[1];
        w.set_captain(Some(starters[0])).await;

        let quiet = w.week(0).await;
        w.snapshot(quiet).await;
        w.stats(benched, quiet, Line { goals: 1, minutes: 60, ..Line::default() })
            .await;
        let quiet_score = w.score(quiet).await;
        assert_eq!(quiet_score.bench_bonus, 0, "no chip, so the bench pays nothing");

        let boosted = w.week(1).await;
        w.snapshot(boosted).await;
        w.play_chip("bench_boost", boosted).await;
        w.stats(benched, boosted, Line { goals: 1, minutes: 60, ..Line::default() })
            .await;
        let boosted_score = w.score(boosted).await;
        assert_eq!(
            boosted_score.bench_bonus, 8,
            "DEF on the bench: 1 goal x 6 + 2 minutes"
        );

        assert_eq!(
            w.displayed_total(benched, true).await,
            8,
            "the bench player's season total must be the boosted week only"
        );

        let rows = w.reconcile().await;
        assert_eq!(rows.len(), 2);
        assert_reconciles(&rows);
        w.close().await;
    }

    /// The captaincy only ever pays a starter. A manager who took the armband off
    /// the bench, or who had no captain set when the week was frozen, must not see
    /// a doubled score for a player who did not start.
    #[tokio::test]
    async fn a_captain_who_did_not_start_is_not_doubled() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "captain", 9400).await;

        let (_starters, bench) = standard_squad(&mut w).await;
        let benched = bench[1];

        // The week is frozen before this team has a captain, so the snapshot records
        // none and scoring falls back to whoever holds the armband today.
        let week = w.week(0).await;
        w.snapshot(week).await;
        w.play_chip("bench_boost", week).await;
        w.set_captain(Some(benched)).await;

        w.stats(benched, week, Line { goals: 1, minutes: 60, ..Line::default() })
            .await;
        let score = w.score(week).await;
        assert_eq!(score.bench_bonus, 8, "the bench pays once under the boost");
        assert_eq!(
            score.captain_bonus, 0,
            "the captain of record did not start, so there is no captain bonus"
        );

        assert_eq!(
            w.displayed_total(benched, true).await,
            8,
            "a captain who did not start must be shown at face value, not doubled"
        );

        let rows = w.reconcile().await;
        assert_eq!(rows.len(), 1);
        assert_reconciles(&rows);
        w.close().await;
    }

    /// A manager who joined mid-season must not be shown points from the weeks that
    /// ran before their team existed, which the gameweek engine refuses to score.
    #[tokio::test]
    async fn a_late_joiner_is_not_shown_points_from_before_they_joined() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "latejoin", 9500).await;

        let (starters, _bench) = standard_squad(&mut w).await;
        w.set_captain(Some(starters[0])).await;

        let before = w.week(0).await;
        let after = w.week(1).await;

        // Move the team's creation between the two weeks: it never played the first.
        sqlx::query("UPDATE fantasy_teams SET created_at = $2::date WHERE id = $1")
            .bind(w.team_id)
            .bind(after.end_date - chrono::Duration::days(1))
            .execute(&mut *w.tx)
            .await
            .expect("backdate team");

        w.stats(starters[1], before, Line { goals: 2, minutes: 90, ..Line::default() })
            .await;
        w.snapshot(after).await;
        w.stats(starters[1], after, Line { goals: 1, minutes: 90, ..Line::default() })
            .await;
        w.score(after).await;

        assert_eq!(
            w.displayed_total(starters[1], false).await,
            8,
            "only the week the manager actually played: 1 goal x 6 + 2 minutes"
        );

        let rows = w.reconcile().await;
        assert_reconciles(&rows);
        w.close().await;
    }

    fn position_of(text: &str) -> PlayerPosition {
        all_positions()
            .into_iter()
            .find(|(name, _)| *name == text)
            .map(|(_, pos)| pos)
            .expect("known position")
    }

    /// One stat's contribution, obtained by running the canonical engine with
    /// only that stat set. Deriving each column this way rather than restating
    /// the rates means the printed breakdown cannot drift from the rules.
    fn only(position: &PlayerPosition, field: impl FnOnce(&mut Line)) -> i32 {
        let mut isolated = Line::default();
        field(&mut isolated);
        PointsEngine::calculate(
            position,
            isolated.goals,
            isolated.assists,
            isolated.clean_sheets,
            isolated.saves,
            isolated.penalty_saves,
            isolated.own_goals,
            isolated.penalty_misses,
            isolated.regular_fouls,
            isolated.serious_fouls,
            isolated.minutes,
        )
    }

    fn engine_total(position: &PlayerPosition, l: Line) -> i32 {
        PointsEngine::calculate(
            position,
            l.goals,
            l.assists,
            l.clean_sheets,
            l.saves,
            l.penalty_saves,
            l.own_goals,
            l.penalty_misses,
            l.regular_fouls,
            l.serious_fouls,
            l.minutes,
        )
    }

    /// Prints the arithmetic behind one manager's gameweek, player by player.
    /// This is the derivation a manager is shown, so it has to be checkable by
    /// hand: every column is one stat's contribution at the position that player
    /// was played in, and the columns add up to the player's line.
    #[tokio::test]
    async fn point_derivation_is_checkable_by_hand() {
        let Some(pool) = pool().await else {
            eprintln!("skipping: DATABASE_URL not set or unreachable");
            return;
        };
        let mut w = World::open(&pool, "derivation", 9600).await;

        let (starters, bench) = standard_squad(&mut w).await;
        let captain = starters[3];
        w.set_captain(Some(captain)).await;

        let week = w.week(0).await;
        w.snapshot(week).await;
        w.play_chip("bench_boost", week).await;

        // (player, the role they were played in this week, their stat line)
        let squad = [
            (starters[0], "GK", false,
             Line { clean_sheets: 1, saves: 7, minutes: 60, ..Line::default() }),
            (starters[1], "DEF", false,
             Line { goals: 1, clean_sheets: 1, minutes: 60, ..Line::default() }),
            (starters[2], "DEF", false,
             Line { clean_sheets: 1, regular_fouls: 2, minutes: 60, ..Line::default() }),
            (starters[3], "MID", false,
             Line { goals: 2, assists: 1, minutes: 90, ..Line::default() }),
            (starters[4], "MID", false,
             Line { assists: 2, minutes: 20, ..Line::default() }),
            (starters[5], "FWD", false,
             Line { goals: 1, own_goals: 1, minutes: 45, ..Line::default() }),
            (bench[0], "GK", true, Line::default()),
            (bench[1], "DEF", true,
             Line { goals: 1, minutes: 40, ..Line::default() }),
            (bench[2], "FWD", true,
             Line { assists: 1, serious_fouls: 1, minutes: 50, ..Line::default() }),
        ];
        for (player, _, _, line) in squad {
            w.stats(player, week, line).await;
        }

        let score = w.score(week).await;

        println!("\ngameweek {} — Bench Boost played, captain doubled", week.number);
        println!(
            "{:<20} {:<4} {:>6} {:>7} {:>4} {:>6} {:>5} {:>5} {:>4} {:>6}",
            "player", "as", "goals", "assist", "cs", "saves", "mins", "neg", "x", "total"
        );

        let mut starter_sum = 0i64;
        let mut bench_sum = 0i64;
        let mut captain_own = 0i64;

        for (player_id, played_as, benched, line) in squad {
            let position = position_of(played_as);
            let name: String = sqlx::query_scalar("SELECT name FROM players WHERE id = $1")
                .bind(player_id)
                .fetch_one(&mut *w.tx)
                .await
                .expect("player name");

            let goals = only(&position, |i| i.goals = line.goals);
            let assists = only(&position, |i| i.assists = line.assists);
            let cs = only(&position, |i| i.clean_sheets = line.clean_sheets);
            let saves = only(&position, |i| {
                i.saves = line.saves;
                i.penalty_saves = line.penalty_saves;
            });
            let mins = only(&position, |i| i.minutes = line.minutes);
            let neg = only(&position, |i| {
                i.own_goals = line.own_goals;
                i.penalty_misses = line.penalty_misses;
                i.regular_fouls = line.regular_fouls;
                i.serious_fouls = line.serious_fouls;
            });

            let base = engine_total(&position, line);
            assert_eq!(
                goals + assists + cs + saves + mins + neg,
                base,
                "{name}'s columns must add up to their line"
            );

            // The captaincy only pays a starter.
            let multiplier = if player_id == captain && !benched { 2 } else { 1 };
            let total = base * multiplier;

            println!(
                "{:<20} {:<4} {:>6} {:>7} {:>4} {:>6} {:>5} {:>5} {:>4} {:>6}",
                name, played_as, goals, assists, cs, saves, mins, neg, multiplier, total
            );

            if benched {
                bench_sum += base as i64;
            } else {
                starter_sum += base as i64;
                if player_id == captain {
                    captain_own = base as i64;
                }
            }
        }

        println!(
            "\nstarters {starter_sum} + captain bonus {captain_own} + bench (Bench Boost) \
             {bench_sum} = gross {}",
            score.gross_points
        );
        println!(
            "engine: starters {} + captain bonus {} + bench {} = gross {} (stored)",
            score.starter_base, score.captain_bonus, score.bench_bonus, score.gross_points
        );

        assert_eq!(score.starter_base, starter_sum, "starter base");
        assert_eq!(score.captain_bonus, captain_own, "captain bonus");
        assert_eq!(score.bench_bonus, bench_sum, "bench bonus");
        assert_eq!(
            score.gross_points as i64,
            starter_sum + captain_own + bench_sum,
            "gross"
        );

        // What the squad page shows each player must be exactly their line above.
        for (player_id, played_as, benched, line) in squad {
            let position = position_of(played_as);
            let expected =
                engine_total(&position, line) * if player_id == captain && !benched { 2 } else { 1 };
            assert_eq!(
                w.displayed_total(player_id, benched).await,
                expected,
                "squad page must show the same number the derivation does"
            );
        }

        let rows = w.reconcile().await;
        assert_eq!(rows.len(), 1);
        print_table(&rows);
        assert_reconciles(&rows);
        w.close().await;
    }

}
