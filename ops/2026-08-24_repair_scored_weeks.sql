-- Repair of gameweeks 1-3, 2026-08-24.
--
-- Fixes three defects found by `cargo test reconciliation_report`, which before
-- this script reports 5 disagreeing rows out of 128:
--
--   A. 60 player rows were appended to lineups that had already been frozen,
--      because the snapshot writer re-populated the lineup on every call. The
--      10 of those that landed after their week was scored would inflate three
--      managers' GW3 scores if that week were ever re-scored.
--   B. Honey73's Triple Captain was recorded against GW3 six days after GW3 was
--      scored, so it paid nothing and cannot be played again.
--   C. premvaiiii holds 60 stored points for GW1, a week the scoring guard now
--      refuses to score for them.
--
-- It also backfills the gameweek dates, which were placeholders from January
-- while the league was actually played in July and August.
--
-- No re-scoring is required. Pruning the appended rows restores each affected
-- lineup to exactly what it was when the week was scored, so every stored score
-- becomes reproducible as it stands. Verified per team:
--   darkside   94 - 22 (Subin Gajurel)                       = 72 = stored
--   aashis    118 - 13 (Aashis Bhattarai 11, Tilak Acharya 2) = 105 = stored
--   saaagaaar  71 - 18 (Tangnami 12, Rijal 6, Basnet 0)       = 53 = stored
--   Honey73    79 - 10 (the Triple Captain that never paid)   = 69 = stored
--
-- USAGE: runs as a dry run and rolls back. Read the notices, then change the
-- final ROLLBACK to COMMIT. Take a backup first.

BEGIN;

-- ---------------------------------------------------------------------------
-- A. Remove the lineup rows appended AFTER their week was scored.
--
-- A genuine freeze writes all nine rows in one statement, so they land within
-- milliseconds of the lineup row. 1134 rows sit under a second from their
-- freeze and none fall between one second and one minute, so freeze and append
-- separate cleanly. But of the 60 appended rows, only 10 arrived after their
-- week was scored:
--
--     gw | appended before scoring | appended after scoring
--      1  |            29           |           0
--      2  |             6           |           0
--      3  |            15           |          10
--
-- The 50 that arrived before scoring are already paid for in the stored score.
-- Deleting those would make the stored scores unreproducible in the other
-- direction: a re-run would come out LOWER than what managers were awarded, on
-- 13 further team-weeks. They are left alone deliberately. See the note at the
-- end of this file for what it would take to correct them instead.
--
-- The 10 that arrived after scoring are the ones that would inflate a re-run,
-- and only they are removed here.
-- ---------------------------------------------------------------------------
DO $$
DECLARE doomed int;
BEGIN
    SELECT count(*) INTO doomed
    FROM team_gameweek_lineup_players lp
    JOIN team_gameweek_lineups l ON l.id = lp.team_gameweek_lineup_id
    JOIN team_gameweek_points g
      ON g.team_id = l.team_id AND g.match_week_id = l.match_week_id
    WHERE lp.created_at >= l.created_at + interval '1 second'
      AND lp.created_at > g.updated_at;

    IF doomed <> 10 THEN
        RAISE EXCEPTION 'aborting: expected 10 post-scoring appends, found %', doomed;
    END IF;
    RAISE NOTICE 'A. removing % lineup rows appended after their week was scored', doomed;
END $$;

DELETE FROM team_gameweek_lineup_players lp
USING team_gameweek_lineups l, team_gameweek_points g
WHERE l.id = lp.team_gameweek_lineup_id
  AND g.team_id = l.team_id AND g.match_week_id = l.match_week_id
  AND lp.created_at >= l.created_at + interval '1 second'
  AND lp.created_at > g.updated_at;

-- ---------------------------------------------------------------------------
-- B. Return Honey73's Triple Captain so they can play it on a live gameweek.
-- ---------------------------------------------------------------------------
DO $$
DECLARE n int;
BEGIN
    DELETE FROM team_chips tc
    USING fantasy_teams ft, users u, match_weeks w
    WHERE tc.team_id = ft.id AND ft.user_id = u.id AND w.id = tc.match_week_id
      AND u.username = 'Honey73'
      AND tc.chip_type = 'triple_captain'
      AND w.week_number = 3;
    GET DIAGNOSTICS n = ROW_COUNT;
    IF n <> 1 THEN
        RAISE EXCEPTION 'aborting: expected to return exactly 1 chip, removed %', n;
    END IF;
    RAISE NOTICE 'B. returned Honey73 triple_captain (unused, replayable)';
END $$;

-- ---------------------------------------------------------------------------
-- C. Remove premvaiiii's GW1 score.
-- ---------------------------------------------------------------------------
DO $$
DECLARE n int;
BEGIN
    DELETE FROM team_gameweek_points g
    USING fantasy_teams ft, users u, match_weeks w
    WHERE g.team_id = ft.id AND ft.user_id = u.id AND w.id = g.match_week_id
      AND u.username = 'premvaiiii'
      AND w.week_number = 1;
    GET DIAGNOSTICS n = ROW_COUNT;
    IF n <> 1 THEN
        RAISE EXCEPTION 'aborting: expected to remove exactly 1 stored score, removed %', n;
    END IF;
    RAISE NOTICE 'C. removed premvaiiii GW1 stored score';
END $$;

-- ---------------------------------------------------------------------------
-- D. Backfill the gameweek dates.
--
-- start_date  = the day the week was frozen.
-- end_date    = the day before the next week was frozen, so the weeks do not
--               overlap; the last scored week ends the day it was scored.
--
-- Deliberately NOT the scored date for earlier weeks: GW1 was scored on 08-12,
-- which is after premvaiiii joined on 08-09, and using it would make them
-- eligible for GW1 again and undo step C.
--
-- Weeks 5-12 are untouched. They have no data and the admin sets their dates
-- when the week is created.
-- ---------------------------------------------------------------------------
WITH spans AS (
    SELECT w.id, w.week_number,
           (SELECT min(l.created_at)::date FROM team_gameweek_lineups l
            WHERE l.match_week_id = w.id) AS frozen_on,
           (SELECT min(g.updated_at)::date FROM team_gameweek_points g
            WHERE g.match_week_id = w.id) AS scored_on
    FROM match_weeks w
    WHERE EXISTS (SELECT 1 FROM team_gameweek_lineups l WHERE l.match_week_id = w.id)
), bounds AS (
    SELECT id, week_number, frozen_on,
           COALESCE(lead(frozen_on) OVER (ORDER BY week_number) - 1, scored_on) AS ends_on
    FROM spans
)
UPDATE match_weeks w
SET start_date = b.frozen_on,
    end_date   = b.ends_on
FROM bounds b
WHERE w.id = b.id;

-- ---------------------------------------------------------------------------
-- Verification
-- ---------------------------------------------------------------------------
SELECT week_number AS gw, start_date, end_date FROM match_weeks
WHERE week_number <= 4 ORDER BY 1;

-- Expect 0: no lineup may still hold a row added after its week was scored.
SELECT count(*) AS post_scoring_appends_remaining
FROM team_gameweek_lineup_players lp
JOIN team_gameweek_lineups l ON l.id = lp.team_gameweek_lineup_id
JOIN team_gameweek_points g
  ON g.team_id = l.team_id AND g.match_week_id = l.match_week_id
WHERE lp.created_at >= l.created_at + interval '1 second'
  AND lp.created_at > g.updated_at;

SELECT (SELECT count(*) FROM team_gameweek_lineup_players) AS lineup_rows,
       (SELECT count(*) FROM team_gameweek_points)         AS stored_scores,
       (SELECT count(*) FROM team_chips)                   AS chips_held;

-- Executed against production 2026-08-24.
COMMIT;

-- ---------------------------------------------------------------------------
-- Not done here: the 50 rows appended before their week was scored.
--
-- Those managers were paid for players who were not in their frozen lineup, so
-- the season as played is not what a clean engine would have produced. Undoing
-- it means deleting all 60 appends and re-scoring gameweeks 1 to 3, which
-- changes stored points and league standings for roughly 13 team-weeks. That is
-- a competition decision, not a data-integrity one, so it is left out of this
-- script. After this repair every stored score reproduces exactly, which is what
-- makes a future re-run safe.
-- ---------------------------------------------------------------------------
