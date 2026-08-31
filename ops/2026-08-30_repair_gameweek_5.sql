-- Repair of gameweek 5, 2026-08-30.
--
-- Gameweek 5 was scored from the squads managers held on Monday 2026-08-24,
-- not the ones they held at the Saturday deadline. Ten of thirty-two managers
-- are affected. This script rebuilds gameweek 5's lineups from the squads that
-- were actually in place at the deadline, so that re-scoring the week through
-- the normal admin path pays what should have been paid.
--
-- IT DOES NOT RE-SCORE. Rewriting the lineups is step 1 of 2; see USAGE.
--
--
-- WHAT HAPPENED
--
-- Gameweek 5 opened at 2026-08-24 13:34 ET and `snapshot_all_lineups` froze
-- every squad at that instant. The freeze was final: `snapshot_team_lineup` is
-- first-write-wins, so the `snapshot_team_lineup_if_missing` call that ran
-- before every later save did nothing. The deadline was the end of Saturday
-- 2026-08-29 — six days later. Everything managers did in between was written
-- to `team_players`, which the league view and the scoring engine never read.
--
-- Six of the eight transfers recorded against gameweek 5 were made on Saturday
-- 2026-08-29 between 20:32 and 23:31 ET, inside the deadline. None of them
-- reached the lineup they were made for.
--
--
-- HOW THE DEADLINE SQUAD IS RECONSTRUCTED
--
-- From `team_players` as it stands, with one correction (section A).
--
-- The evidence that this is the deadline squad, and the limit of that evidence:
--
--   * Selection was locked Sunday 2026-08-30 00:00-12:00 ET. `force_unlock` is
--     false and was last touched 2026-08-01, so the lock held. `set_team_players`,
--     `transfer_player` and `activate_chip` all refuse while locked, so no
--     squad could change in that window.
--   * That leaves Sunday 12:00-18:31 ET, between the reopening and gameweek 6
--     opening. Exactly one write is recorded in it: PIRAT3S's transfer at
--     18:03 ET, corrected in section A.
--   * Gameweek 6's lineups were frozen at 18:31 ET and are byte-identical to
--     all 32 current squads, so nothing has changed since.
--   * All four gameweek 5 chips were played on the Saturday, inside the
--     deadline. They need no correction.
--
--   * LIMIT: `team_players` has no timestamps, so a manager who only
--     REARRANGED — benched a starter, moved the armband, changed a position —
--     between Sunday noon and 18:31 ET would leave no trace, and this script
--     would treat that rearrangement as their deadline squad. Postgres logs
--     for the window record no statement-level DML, so this cannot be ruled
--     out from the data. It is a 6.5-hour window, after the matches and before
--     any new gameweek existed to plan for.
--
-- Two managers LOSE points under this repair (darkside -14, Kamakazek -4) and
-- six gain. That is expected: a stale lineup is not biased, and the repair is
-- for correctness, not for anyone's benefit.
--
--
-- USAGE
--
-- RUN THIS EARLY. Section 0 pins the squads to the state the repair was
-- computed from, and any manager changing their gameweek 6 team will trip it.
-- The intended order is: deploy (which applies migration 021), then this
-- script, then step 4, back to back.
--
--   1. Take a backup.
--   2. Run this file. It is a dry run and rolls back. Read the notices and the
--      verification output.
--   3. Change the final ROLLBACK to COMMIT and run it again.
--   4. Re-score gameweek 5 through the admin UI by submitting its stats
--      unchanged. That runs the real engine over the corrected lineups; no
--      scoring arithmetic is reimplemented here. Gameweek 5 is not the active
--      week, so `close_week_and_open_next` will not wind the league back to it.
--   5. Confirm with `cargo test reconciliation_report -- --nocapture`.
--
--
-- ONE SIDE EFFECT OF STEP 4, WORTH KNOWING BEFORE YOU START
--
-- `submit_week_stats` does two things besides scoring, and they run again on a
-- re-score:
--
--   * `apply_gameweek_price_adjustments` reverses gameweek 5's price moves and
--     re-applies them. The stats are unchanged, so the same three players go up
--     and the same three go down: no net change.
--   * every team's `budget_limit` is set to the cost of the squad it holds NOW.
--     That is what a scoring run always does, but doing it mid-gameweek-6
--     brings it forward: any headroom a manager is currently carrying from
--     price falls disappears a week early. Nobody's squad becomes invalid — the
--     budget is set to what they already hold — but a planned transfer may no
--     longer fit.
--
-- If that matters, capture `SELECT id, budget_limit FROM fantasy_teams` before
-- step 4 and restore it after.

BEGIN;

CREATE TEMP TABLE gw ON COMMIT DROP AS
    SELECT id, week_number, lineup_deadline
    FROM match_weeks WHERE week_number = 5;

CREATE TEMP TABLE successor ON COMMIT DROP AS
    SELECT id, week_number FROM match_weeks WHERE week_number = 6;

DO $$
DECLARE d timestamptz;
BEGIN
    SELECT lineup_deadline INTO d FROM gw;
    IF d IS NULL THEN
        RAISE EXCEPTION
            'aborting: gameweek 5 has no lineup_deadline. Run migration 021 first';
    END IF;
    IF d <> '2026-08-30 04:00:00+00'::timestamptz THEN
        RAISE EXCEPTION
            'aborting: gameweek 5 deadline is %, expected 2026-08-30 00:00 ET', d;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM team_gameweek_points WHERE match_week_id = (SELECT id FROM gw)) THEN
        RAISE EXCEPTION 'aborting: gameweek 5 has no stored scores — wrong database?';
    END IF;
    RAISE NOTICE 'gameweek 5 closed at % ET', d AT TIME ZONE 'America/New_York';
END $$;

-- ---------------------------------------------------------------------------
-- 0. Refuse to run if any squad has moved since the reconstruction was checked.
--
-- This is the guard the reconstruction rests on, and it has a short life.
--
-- The reasoning in "HOW THE DEADLINE SQUAD IS RECONSTRUCTED" leans on gameweek
-- 6's lineups being byte-identical to all 32 current squads, which they were
-- when this was written. That evidence does not survive the deploy this script
-- ships with: from then on every save rewrites the live week's lineup, so
-- gameweek 6 tracks `team_players` continuously and stays byte-identical no
-- matter how far the squads move. It stops being a check the moment the code
-- is running.
--
-- So the state is pinned here instead. These fingerprints were taken from
-- production at 2026-08-30 19:20 ET, before the deploy, over exactly the squads
-- the projected deltas were computed from. A manager making a gameweek 6
-- transfer or benching a player changes their fingerprint and this aborts —
-- where every other precondition in this file would have passed happily, since
-- a swap or a bench move preserves both the player count and their
-- distinctness.
--
-- If it aborts, the squads have moved on and gameweek 5's deadline lineups can
-- no longer be recovered from live data. Do not weaken this check to get past
-- it; the whole point of the repair is to stop paying managers for squads they
-- did not pick.
-- ---------------------------------------------------------------------------
CREATE TEMP TABLE expected_squad_fingerprint (username text PRIMARY KEY, md5 text)
    ON COMMIT DROP;

INSERT INTO expected_squad_fingerprint (username, md5) VALUES
  ('aashis', '5a0105d0b8cf27e977ddd0b98a7d39d4'),
  ('Aashsih Tangnami', '25853852849a5aece381b20d52e71526'),
  ('Aayushrjl', '03e0eeb4409d376e1c0d01090bd6eb91'),
  ('Anod crestha ', '1ad832d3a75c905215bfd6ced75c7f00'),
  ('Bishal022', '6a1e4592ff32675f48c6812fb6705689'),
  ('darkside', 'd10d6ad9e1a163d94e0b48c4915c64f7'),
  ('Dipen ', 'b5295cf5a2afb5cfd5e6df7d1356ac71'),
  ('DipUnited', '2891c1d0ff864e4507897f0fafc175f7'),
  ('Ekata Pariwar', 'd38d30b9911d357b7a367c2594aa967f'),
  ('Gajjudada', '35163e674f38cfed6f09a0ee00182252'),
  ('Himalayan Gaint', '21f1e7214d2cfa6a9f561f88dd8b56e7'),
  ('Honey73', 'f081b4808c814e7eac1fe6ec95ea1057'),
  ('Kamakazek', '94ffcf962f272c78f16e41eb1c055894'),
  ('Kaplan', '61b28152b175ba52fdf3a5c9caed5e1d'),
  ('kindbeast', 'da3adc59292587ca2d27731f50520fb7'),
  ('KKFantasy', '916b8be44d6e46321bcc482e08ed6087'),
  ('Mrdash145', '0f93dbee31ecb96a91598889b7c08a6c'),
  ('Mrr highest scorer', 'b4bc1a3145cf2f53b93b3c349780734a'),
  ('PIRAT3S ', 'd7a358c555d6247e70259072bf5675c2'),
  ('Poison', '5c838f413b537b6740227470d1ffe311'),
  ('Pratik', '031fc6e707457e93777de881a8353b69'),
  ('premvaiiii', '116ef054c9ed586e60222f90bdd06df8'),
  ('Rajeev07', '33609f136fd325c8e73f3d7c9aeaccad'),
  ('Rajeev7', '24facedd8200ebe4507449f49e88c395'),
  ('saaagaaar', '38be495a715ed71b7a1a25162d2651b8'),
  ('Suprab Rajbhandari', 'fe349ba2f3d6a631683afb62ec3b953b'),
  ('The best', '527a8550bf20ce0bf2c5010e64925ac7'),
  ('Thesidearth', '64837fde99aae564500ad1293e90cb13'),
  ('Tilak777', '4e072057225890de0536a79f51657338'),
  ('wheregmis', 'a8f2965df43c0bb1c12384fbeff6afd9'),
  ('Yukesh10', '7a36243beb3a6b35bf6ce81d17cabaa0'),
  ('Zindagi Rocks', '475176c18ac29f86059deb6c5bafe2e5');

DO $$
DECLARE
    drifted   int;
    unknown   int;
    later_tr  int;
BEGIN
    SELECT count(*) INTO later_tr
    FROM transfers tr JOIN match_weeks w ON w.id = tr.match_week_id
    WHERE w.week_number > 5;

    IF later_tr > 0 THEN
        RAISE EXCEPTION
            'aborting: % transfer(s) recorded against a gameweek after 5. Squads have '
            'moved on and the gameweek 5 deadline lineups can no longer be reconstructed',
            later_tr;
    END IF;

    SELECT count(*) INTO drifted
    FROM (
        SELECT u.username,
               md5(string_agg(tp.player_id::text || ':' || tp.is_bench::text || ':' ||
                              COALESCE(tp.assigned_position::text, '-'), '|'
                              ORDER BY tp.player_id)) AS actual
        FROM team_players tp
        JOIN fantasy_teams ft ON ft.id = tp.team_id
        JOIN users u ON u.id = ft.user_id
        GROUP BY u.username
    ) now_
    JOIN expected_squad_fingerprint e ON e.username = now_.username
    WHERE e.md5 <> now_.actual;

    SELECT count(*) INTO unknown
    FROM fantasy_teams ft
    JOIN users u ON u.id = ft.user_id
    WHERE NOT EXISTS (SELECT 1 FROM expected_squad_fingerprint e WHERE e.username = u.username);

    IF drifted > 0 OR unknown > 0 THEN
        RAISE EXCEPTION
            'aborting: % squad(s) changed since 2026-08-30 19:20 ET and % team(s) are '
            'not in the fingerprint list. The reconstruction is no longer valid',
            drifted, unknown;
    END IF;

    RAISE NOTICE '0. all 32 squads match the state the repair was computed from';
END $$;

-- ---------------------------------------------------------------------------
-- A. Move the post-deadline transfer to the gameweek it belongs to.
--
-- PIRAT3S swapped Aashis Bhattarai for Nirmal Gurung at 18:03 ET on the
-- Sunday: after the deadline, after the matches, 28 minutes before gameweek 5
-- was scored. It could not affect gameweek 5, but it still counted as their
-- second transfer of the week and charged them -4.
--
-- The intent was plainly gameweek 6 — they still hold Nirmal Gurung — so the
-- transfer is re-billed there rather than deleted. Gameweek 6 is unscored, so
-- this only changes what its hit will be.
-- ---------------------------------------------------------------------------
CREATE TEMP TABLE late_transfers ON COMMIT DROP AS
    SELECT tr.id, tr.team_id, tr.player_out_id, tr.player_in_id, tr.created_at
    FROM transfers tr, gw
    WHERE tr.match_week_id = gw.id
      AND tr.created_at >= gw.lineup_deadline;

DO $$
DECLARE n int;
BEGIN
    SELECT count(*) INTO n FROM late_transfers;
    IF n <> 1 THEN
        RAISE EXCEPTION 'aborting: expected exactly 1 post-deadline transfer, found %', n;
    END IF;
    RAISE NOTICE 'A. re-billing % post-deadline transfer to gameweek 6', n;
END $$;

UPDATE transfers tr
SET match_week_id = (SELECT id FROM successor)
FROM late_transfers lt
WHERE tr.id = lt.id;

-- ---------------------------------------------------------------------------
-- B. Rebuild gameweek 5's lineups from the deadline squads.
--
-- The deadline squad is the current squad with every post-deadline transfer
-- reversed: the player who came in is replaced by the player who went out, in
-- the same slot, with the same role.
--
-- DELETE then INSERT. `ON CONFLICT DO NOTHING` protects the rows already there
-- while still appending the rest, which is how 60 stray players accumulated in
-- gameweeks 1-3; see ops/2026-08-24_repair_scored_weeks.sql section A.
-- ---------------------------------------------------------------------------
CREATE TEMP TABLE deadline_squad ON COMMIT DROP AS
    SELECT tp.team_id,
           COALESCE(lt.player_out_id, tp.player_id) AS player_id,
           tp.is_bench,
           tp.assigned_position
    FROM team_players tp
    LEFT JOIN late_transfers lt
           ON lt.team_id = tp.team_id AND lt.player_in_id = tp.player_id;

DO $$
DECLARE bad int;
BEGIN
    SELECT count(*) INTO bad
    FROM (SELECT team_id FROM deadline_squad GROUP BY team_id HAVING count(*) <> 9) t;
    IF bad > 0 THEN
        RAISE EXCEPTION 'aborting: % teams do not have exactly 9 players', bad;
    END IF;

    SELECT count(*) INTO bad
    FROM (SELECT team_id, player_id FROM deadline_squad
          GROUP BY team_id, player_id HAVING count(*) > 1) t;
    IF bad > 0 THEN
        RAISE EXCEPTION 'aborting: % duplicated players after reversing transfers', bad;
    END IF;

    RAISE NOTICE 'B. rebuilding % lineups from the deadline squads',
        (SELECT count(DISTINCT team_id) FROM deadline_squad);
END $$;

-- The armband: gameweek 5 kept whichever captain was frozen at the open, and
-- fantasy_teams.captain_id is the one in force now. Same reconstruction rule.
UPDATE team_gameweek_lineups l
SET captain_id = ft.captain_id
FROM fantasy_teams ft, gw
WHERE ft.id = l.team_id AND l.match_week_id = gw.id;

DELETE FROM team_gameweek_lineup_players lp
USING team_gameweek_lineups l, gw
WHERE l.id = lp.team_gameweek_lineup_id
  AND l.match_week_id = gw.id;

INSERT INTO team_gameweek_lineup_players
      (team_gameweek_lineup_id, player_id, is_bench, assigned_position)
SELECT l.id, d.player_id, d.is_bench, d.assigned_position
FROM team_gameweek_lineups l
JOIN gw ON gw.id = l.match_week_id
JOIN deadline_squad d ON d.team_id = l.team_id;

-- ---------------------------------------------------------------------------
-- Verification
-- ---------------------------------------------------------------------------

-- Expect 32 rows of 9. No lineup may have grown or shrunk.
SELECT count(*) AS lineups,
       min(n)   AS smallest,
       max(n)   AS largest
FROM (
    SELECT l.team_id, count(*) AS n
    FROM team_gameweek_lineups l
    JOIN gw ON gw.id = l.match_week_id
    JOIN team_gameweek_lineup_players lp ON lp.team_gameweek_lineup_id = l.id
    GROUP BY l.team_id
) t;

-- Expect 0: no gameweek 5 transfer may remain on the wrong side of the deadline.
SELECT count(*) AS post_deadline_transfers_remaining
FROM transfers tr, gw
WHERE tr.match_week_id = gw.id AND tr.created_at >= gw.lineup_deadline;

-- The stored scores that step 4 will change, and by how much. `expected_gross`
-- mirrors services::points_sql::week_points; it is here to be read, not to be
-- written back. The engine, not this query, produces the new stored score.
SELECT u.username,
       p.total_points AS stored_now,
       x.gross - GREATEST((SELECT count(*) FROM transfers tr
                            WHERE tr.team_id = ft.id AND tr.match_week_id = gw.id) - 1, 0) * 4
                 AS expected_after,
       x.gross - GREATEST((SELECT count(*) FROM transfers tr
                            WHERE tr.team_id = ft.id AND tr.match_week_id = gw.id) - 1, 0) * 4
                 - p.total_points AS delta
FROM fantasy_teams ft
JOIN users u ON u.id = ft.user_id
CROSS JOIN gw
JOIN team_gameweek_points p ON p.team_id = ft.id AND p.match_week_id = gw.id
JOIN LATERAL (
    SELECT SUM(
        CASE WHEN NOT lp.is_bench OR EXISTS (
                SELECT 1 FROM team_chips tc WHERE tc.team_id = ft.id
                 AND tc.match_week_id = gw.id AND tc.chip_type = 'bench_boost')
             THEN (
               CASE COALESCE(lp.assigned_position, pl.position)::text
                 WHEN 'GK'  THEN COALESCE(pp.goals, 0) * 10
                 WHEN 'DEF' THEN COALESCE(pp.goals, 0) * 6
                 WHEN 'MID' THEN COALESCE(pp.goals, 0) * 5
                 WHEN 'FWD' THEN COALESCE(pp.goals, 0) * 4 ELSE 0 END
               + COALESCE(pp.assists, 0) * 5
               + CASE COALESCE(lp.assigned_position, pl.position)::text
                   WHEN 'GK' THEN COALESCE(pp.clean_sheets, 0) * 2
                   WHEN 'DEF' THEN COALESCE(pp.clean_sheets, 0) * 2 ELSE 0 END
               + CASE WHEN COALESCE(lp.assigned_position, pl.position)::text = 'GK'
                      THEN COALESCE(pp.saves, 0) / 5 ELSE 0 END
               + COALESCE(pp.penalty_saves, 0) * 8
               + CASE WHEN COALESCE(pp.minutes_played, 0) >= 35 THEN 2
                      WHEN COALESCE(pp.minutes_played, 0) >= 1  THEN 1 ELSE 0 END
               - COALESCE(pp.own_goals, 0) * 2 - COALESCE(pp.penalty_misses, 0) * 2
               - COALESCE(pp.regular_fouls, 0) - COALESCE(pp.serious_fouls, 0) * 3
             ) * CASE WHEN NOT lp.is_bench AND l.captain_id = lp.player_id
                      THEN CASE WHEN EXISTS (
                             SELECT 1 FROM team_chips tc WHERE tc.team_id = ft.id
                              AND tc.match_week_id = gw.id AND tc.chip_type = 'triple_captain')
                           THEN 3 ELSE 2 END
                      ELSE 1 END
             ELSE 0 END
    )::int AS gross
    FROM team_gameweek_lineups l
    JOIN team_gameweek_lineup_players lp ON lp.team_gameweek_lineup_id = l.id
    JOIN players pl ON pl.id = lp.player_id
    LEFT JOIN player_points pp ON pp.player_id = lp.player_id AND pp.match_week_id = gw.id
    WHERE l.team_id = ft.id AND l.match_week_id = gw.id
) x ON true
WHERE x.gross - GREATEST((SELECT count(*) FROM transfers tr
                           WHERE tr.team_id = ft.id AND tr.match_week_id = gw.id) - 1, 0) * 4
      <> p.total_points
ORDER BY delta DESC;

ROLLBACK;
