-- Deploy-day refresh of the live gameweek's lineups, 2026-08-30.
--
-- Run this ONCE, in the same deploy as the deadline-snapshot change.
--
-- WHY IT IS NEEDED
--
-- Before the change, a gameweek's lineups were frozen when the week opened and
-- never moved again. After it, every save rewrites them until the deadline. A
-- manager who saves after the deploy therefore gets the new rule; a manager who
-- saved before it and does not open the app again keeps a lineup frozen at the
-- open. Which rule a manager plays under would depend on whether they happened
-- to touch the app after a deploy they cannot see.
--
-- This script removes that split by bringing every lineup in the live week up
-- to the squad its manager currently holds — which is exactly what the new code
-- would have written on their last save.
--
-- WHEN IT IS SAFE
--
-- Only while the live week is still before its deadline. After the deadline the
-- live squad is no longer the squad that was played, and running this would
-- import post-deadline changes into a settled week — the very thing the new
-- code refuses to do. The guard below enforces that, so a stray re-run after
-- the deadline aborts rather than doing damage.
--
-- At the time of writing gameweek 6 had been open for 27 minutes and its
-- lineups were already byte-identical to all 32 live squads, so this is
-- expected to report 0 changed rows. It is written anyway, because the deploy
-- may not land that promptly, and because "expected to be a no-op" is only
-- worth trusting when something checks.
--
-- USAGE: runs as a dry run and rolls back. Read the notices, then change the
-- final ROLLBACK to COMMIT.

BEGIN;

-- Fail with an explanation rather than a bare "column does not exist": every
-- query below reads lineup_deadline, so a database that has not had migration
-- 021 applied breaks at parse time, before any precondition can report why.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'match_weeks' AND column_name = 'lineup_deadline'
    ) THEN
        RAISE EXCEPTION
            'aborting: match_weeks.lineup_deadline does not exist. Deploy the '
            'backend first — it applies migrations/021_gameweek_lineup_deadline.sql '
            'at boot. Do not apply that file by hand: sqlx tracks applied '
            'migrations in _sqlx_migrations and would try to run it again.';
    END IF;
END $$;

-- ---------------------------------------------------------------------------
-- Preconditions. Any failure here aborts the whole script.
-- ---------------------------------------------------------------------------
DO $$
DECLARE
    live       record;
    n_lineups  int;
BEGIN
    SELECT w.id, w.week_number, w.lineup_deadline
      INTO live
      FROM match_weeks w
     WHERE w.is_active
     LIMIT 2;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'aborting: no active gameweek';
    END IF;

    IF (SELECT count(*) FROM match_weeks WHERE is_active) <> 1 THEN
        RAISE EXCEPTION 'aborting: % gameweeks are active, expected exactly 1',
            (SELECT count(*) FROM match_weeks WHERE is_active);
    END IF;

    IF live.lineup_deadline IS NULL THEN
        -- The column exists (checked above), so 021 has run and its backfill
        -- found nothing to anchor on: this week has no lineups to date it from,
        -- or it was opened by code that predates the deadline.
        RAISE EXCEPTION
            'aborting: gameweek % has no deadline. Re-open it from the admin '
            'console so it gets one, then re-run this',
            live.week_number;
    END IF;

    IF NOW() >= live.lineup_deadline THEN
        RAISE EXCEPTION
            'aborting: gameweek % passed its deadline at %. The live squad is no '
            'longer what was played, so refreshing would import post-deadline '
            'changes into a settled week',
            live.week_number, live.lineup_deadline;
    END IF;

    IF EXISTS (SELECT 1 FROM team_gameweek_points g WHERE g.match_week_id = live.id) THEN
        RAISE EXCEPTION
            'aborting: gameweek % already has stored scores', live.week_number;
    END IF;

    SELECT count(*) INTO n_lineups
      FROM team_gameweek_lineups l WHERE l.match_week_id = live.id;

    IF n_lineups <> (SELECT count(*) FROM fantasy_teams) THEN
        RAISE EXCEPTION
            'aborting: gameweek % has % lineups for % teams. Seed them first',
            live.week_number, n_lineups, (SELECT count(*) FROM fantasy_teams);
    END IF;

    RAISE NOTICE 'gameweek % is live, unscored, and closes at % — refreshing % lineups',
        live.week_number, live.lineup_deadline, n_lineups;
END $$;

-- ---------------------------------------------------------------------------
-- The live week, and how far its lineups have drifted from the squads.
--
-- One row per player that is in the lineup but not the squad, or the other way
-- round, counting a change of bench or position as a difference in both
-- directions. Set difference rather than an outer join: a player missing from
-- one side has no row to join to, and chaining two full outer joins multiplies
-- rows instead of pairing them.
-- ---------------------------------------------------------------------------
CREATE TEMP TABLE live_week ON COMMIT DROP AS
    SELECT id, week_number, lineup_deadline FROM match_weeks WHERE is_active;

CREATE TEMP VIEW live_week_drift AS
WITH lineup AS (
    SELECT l.team_id, lp.player_id, lp.is_bench, lp.assigned_position
    FROM team_gameweek_lineups l
    JOIN team_gameweek_lineup_players lp ON lp.team_gameweek_lineup_id = l.id
    WHERE l.match_week_id = (SELECT id FROM live_week)
), squad AS (
    SELECT tp.team_id, tp.player_id, tp.is_bench, tp.assigned_position
    FROM team_players tp
)
SELECT 'in the lineup, not the squad' AS side, d.* FROM (
    SELECT * FROM lineup EXCEPT SELECT * FROM squad) d
UNION ALL
SELECT 'in the squad, not the lineup', d.* FROM (
    SELECT * FROM squad EXCEPT SELECT * FROM lineup) d;

-- What is about to change, before it changes.
SELECT u.username, d.side, p.name AS player, d.is_bench, d.assigned_position
FROM live_week_drift d
JOIN fantasy_teams ft ON ft.id = d.team_id
JOIN users u ON u.id = ft.user_id
JOIN players p ON p.id = d.player_id
ORDER BY u.username, d.side, p.name;

-- ---------------------------------------------------------------------------
-- The refresh. Delete then insert, exactly as scoring::refresh_team_lineup
-- does. Appending is what put 60 stray players into gameweeks 1-3; see
-- ops/2026-08-24_repair_scored_weeks.sql section A.
-- ---------------------------------------------------------------------------
UPDATE team_gameweek_lineups l
SET captain_id = ft.captain_id
FROM fantasy_teams ft
WHERE ft.id = l.team_id
  AND l.match_week_id = (SELECT id FROM live_week);

DELETE FROM team_gameweek_lineup_players lp
USING team_gameweek_lineups l
WHERE l.id = lp.team_gameweek_lineup_id
  AND l.match_week_id = (SELECT id FROM live_week);

INSERT INTO team_gameweek_lineup_players
      (team_gameweek_lineup_id, player_id, is_bench, assigned_position)
SELECT l.id, tp.player_id, tp.is_bench, tp.assigned_position
FROM team_gameweek_lineups l
JOIN team_players tp ON tp.team_id = l.team_id
WHERE l.match_week_id = (SELECT id FROM live_week);

-- ---------------------------------------------------------------------------
-- Verification. Expect 0 rows: every lineup in the live week must now match
-- its squad exactly, in membership, bench and position.
-- ---------------------------------------------------------------------------
SELECT u.username, d.side, p.name AS player, d.is_bench, d.assigned_position
FROM live_week_drift d
JOIN fantasy_teams ft ON ft.id = d.team_id
JOIN users u ON u.id = ft.user_id
JOIN players p ON p.id = d.player_id
ORDER BY u.username, d.side, p.name;

-- Expect 0: no captain may disagree with the squad it was taken from.
SELECT count(*) AS captains_out_of_step
FROM team_gameweek_lineups l
JOIN fantasy_teams ft ON ft.id = l.team_id
WHERE l.match_week_id = (SELECT id FROM live_week)
  AND l.captain_id IS DISTINCT FROM ft.captain_id;

SELECT w.week_number,
       w.lineup_deadline,
       count(DISTINCT l.team_id) AS lineups,
       count(lp.id)              AS players
FROM live_week w
JOIN team_gameweek_lineups l ON l.match_week_id = w.id
JOIN team_gameweek_lineup_players lp ON lp.team_gameweek_lineup_id = l.id
GROUP BY w.week_number, w.lineup_deadline;

ROLLBACK;
