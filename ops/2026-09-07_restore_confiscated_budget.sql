-- Give back the budget the old carry-forward confiscated, 2026-09-07.
--
-- Until the fix that accompanies this script, every scoring run ended with
--
--     UPDATE fantasy_teams SET budget_limit = <cost of the squad it holds>
--
-- which set each manager's spending power *to* their squad value instead of
-- moving it *by* what the week did to that squad. The difference between the
-- two — the money a manager had deliberately not spent — was taken every time a
-- gameweek was scored. A manager holding a $67.95 squad against a $70.00 budget
-- started the next week with $67.95 and an empty bank.
--
-- It also ratcheted. Selling a $9.00 player for a $5.00 one banked $4.00; the
-- next scoring run swallowed that too, and the $9.00 player could never be
-- bought back. Managers who spent to the last cent lost nothing; only the ones
-- who saved were charged.
--
-- The code no longer does this. This script repairs the balances it already
-- damaged, which will not correct themselves.
--
--
-- HOW THE INTENDED BUDGET IS RECONSTRUCTED
--
-- Under the rule as it should always have worked, a budget is $70.00 at entry
-- and moves only when a player the manager holds changes price:
--
--     budget = 70.00 + (every price move that landed on their squad)
--
-- Both halves of that are still on record. `gameweek_price_adjustments` holds
-- each scored week's per-player moves, already clamped to the price floor, and
-- was never rewritten by the confiscation. `team_gameweek_lineups` /
-- `team_gameweek_lineup_players` hold the nine players each manager was frozen
-- into that week. Joining the two says exactly what each week's prices did to
-- each squad.
--
-- Two things this does not reach, both of which round in the manager's favour
-- or not at all:
--
--   * A week scored before `team_gameweek_lineups` existed has no snapshot, so
--     its moves count as zero. It has no `gameweek_price_adjustments` rows
--     either, so the two agree and nothing is lost.
--   * The snapshot is frozen at the deadline, while prices moved at scoring.
--     Transfers are closed across that window, so the squad is the same one.
--
--   * Lineup snapshots only became reliable at `f5695ca`, which moved the
--     freeze from a week's open to its deadline. For weeks frozen before that,
--     the snapshot is the squad as it stood when the week opened; a manager who
--     transferred mid-week has that week's move attributed to the player they
--     started it with.
--
-- Every affected team's budget goes UP: the confiscation only ever took. No
-- squad can become unaffordable as a result, and the guard below stops the
-- script rather than let a reconstruction that disagrees take money.
--
-- USAGE: runs as a dry run and rolls back. Read the notices, satisfy yourself
-- that the per-team numbers are right, then change the final ROLLBACK to
-- COMMIT. Take a backup first. Safe to run more than once — it computes an
-- absolute target, so a second run is a no-op.

BEGIN;

-- What each team's budget should be, had it never been confiscated.
CREATE TEMP TABLE intended_budget ON COMMIT DROP AS
SELECT
    ft.id                                        AS team_id,
    ft.name,
    ft.budget_limit                              AS current_budget,
    70.00 + COALESCE(SUM(adj.delta), 0)          AS intended_budget
FROM fantasy_teams ft
LEFT JOIN team_gameweek_lineups lineup
       ON lineup.team_id = ft.id
LEFT JOIN team_gameweek_lineup_players held
       ON held.team_gameweek_lineup_id = lineup.id
LEFT JOIN gameweek_price_adjustments adj
       ON adj.match_week_id = lineup.match_week_id
      AND adj.player_id = held.player_id
GROUP BY ft.id, ft.name, ft.budget_limit;

-- What each team can currently afford, so the report can show that restoring
-- the money never leaves anyone holding a squad they could not buy.
CREATE TEMP TABLE squad_cost ON COMMIT DROP AS
SELECT ft.id AS team_id, COALESCE(SUM(p.price), 0) AS cost
FROM fantasy_teams ft
LEFT JOIN team_players tp ON tp.team_id = ft.id
LEFT JOIN players p ON p.id = tp.player_id
GROUP BY ft.id;

DO $$
DECLARE
    r RECORD;
    total numeric := 0;
BEGIN
    RAISE NOTICE '%', rpad('team', 24) || rpad('now', 10) || rpad('should be', 12)
                      || rpad('returned', 11) || 'squad';
    FOR r IN
        SELECT i.name, i.current_budget, i.intended_budget,
               i.intended_budget - i.current_budget AS returned,
               s.cost
        FROM intended_budget i
        JOIN squad_cost s ON s.team_id = i.team_id
        ORDER BY i.intended_budget - i.current_budget DESC, i.name
    LOOP
        RAISE NOTICE '%', rpad(r.name, 24) || rpad(r.current_budget::text, 10)
                          || rpad(r.intended_budget::text, 12)
                          || rpad(r.returned::text, 11) || r.cost::text;
        total := total + GREATEST(r.returned, 0);
    END LOOP;
    RAISE NOTICE '--';
    RAISE NOTICE 'returning % in total', total;
END $$;

-- The confiscation only ever took money, so a reconstruction that hands any
-- team LESS than it holds today means the reconstruction is wrong, not the
-- balance. Stop rather than take more.
DO $$
DECLARE
    losers int;
BEGIN
    SELECT COUNT(*) INTO losers
    FROM intended_budget
    WHERE intended_budget < current_budget;

    IF losers > 0 THEN
        RAISE EXCEPTION
            '% team(s) would lose budget. The reconstruction disagrees with the '
            'stored balances — investigate before committing.', losers;
    END IF;
END $$;

UPDATE fantasy_teams ft
SET budget_limit = i.intended_budget
FROM intended_budget i
WHERE ft.id = i.team_id
  AND ft.budget_limit <> i.intended_budget;

-- Change to COMMIT once the notices above look right.
ROLLBACK;
