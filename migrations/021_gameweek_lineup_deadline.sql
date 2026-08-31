-- The instant a gameweek's lineups stop counting.
--
-- The deadline is the end of Saturday Eastern, which is the Sunday 00:00
-- America/New_York that follows it. It is stored as an absolute instant, set
-- from the clock when the week is opened, so every guard can compare it with
-- NOW() in the same statement that writes.
--
-- Deliberately NOT derived from start_date/end_date. Those hold seed
-- placeholders for every week an admin has not hand-edited: gameweek 6 is the
-- live week as this ships and its end_date still reads 2026-02-11, and
-- ops/2026-08-24_repair_scored_weeks.sql had to reconstruct gameweeks 1-4 from
-- lineup freeze times for the same reason.
--
-- NULL means no deadline is recorded, which every guard reads as "closed":
-- NOW() < NULL is NULL, not true. A week that has never been opened therefore
-- behaves exactly as the code did before this column existed.
ALTER TABLE match_weeks
    ADD COLUMN IF NOT EXISTS lineup_deadline TIMESTAMPTZ;

-- Backfill from what actually happened: min(team_gameweek_lineups.created_at)
-- is the moment a week was frozen, the same anchor the August repair used, and
-- match_weeks carries no created_at of its own.
--
-- LEAST clamps the raw rule against the moment the week was scored and the
-- moment its successor opened, so a deadline can never fall after the week was
-- already settled. Gameweek 4 needs this: it was opened Sunday 2026-08-23
-- 21:12 ET and scored 101 minutes later, and the raw rule would hand it a
-- deadline a week after it closed. LEAST ignores NULLs, so the open week keeps
-- the raw value.
--
-- Verified against production before shipping. The backfill resolves
-- gameweek 5 to Sunday 2026-08-30 00:00 ET — the end of Saturday the 29th, the
-- deadline its managers actually played to — and gameweek 6 to Sunday
-- 2026-09-06 00:00 ET.
WITH opened AS (
    SELECT w.id, w.week_number,
           (SELECT min(l.created_at) FROM team_gameweek_lineups l
             WHERE l.match_week_id = w.id) AS opened_at,
           (SELECT min(g.updated_at) FROM team_gameweek_points g
             WHERE g.match_week_id = w.id) AS scored_at
    FROM match_weeks w
), spans AS (
    SELECT id, opened_at, scored_at,
           lead(opened_at) OVER (ORDER BY week_number) AS next_opened_at
    FROM opened
)
UPDATE match_weeks w
SET lineup_deadline = LEAST(
        -- The next Sunday 00:00 Eastern strictly after the week opened.
        -- EXTRACT(DOW) is 0 on Sunday, so 7 - DOW is 7 on a Sunday and 1 on a
        -- Saturday: always a strictly later calendar date, whatever the hour.
        (
            (
                (s.opened_at AT TIME ZONE 'America/New_York')::date
                + (7 - EXTRACT(DOW FROM (s.opened_at AT TIME ZONE 'America/New_York'))::int)
            )::timestamp
        ) AT TIME ZONE 'America/New_York',
        s.scored_at,
        s.next_opened_at
    )
FROM spans s
WHERE w.id = s.id
  AND s.opened_at IS NOT NULL
  AND w.lineup_deadline IS NULL;
