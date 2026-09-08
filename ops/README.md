# One-off operational scripts

Each file here repairs something a migration cannot: a specific incident, on
specific rows, at a specific time. They are not migrations and nothing applies
them automatically — someone runs them by hand, once, and that is the point.

## The convention

Every script is written as a **dry run**: it does its work inside a transaction
and ends in `ROLLBACK;`. Read the notices it prints, satisfy yourself the
numbers are right, then change the final `ROLLBACK` to `COMMIT` and run it
again.

A script that has been run against production records it, in the file, above the
`COMMIT`:

```sql
-- Executed against production 2026-08-24.
COMMIT;
```

That line is the only record this project keeps of what has been applied. There
is no table, no migration row, nothing in the database to ask. If you run one,
commit the marker in the same change — otherwise the next person cannot tell a
script that ran from one that never did, and several of these are unsafe or
meaningless to run twice.

## What has been run

Per the markers in the files themselves, as of 2026-09-08:

| Script | Purpose | State |
| --- | --- | --- |
| `2026-08-24_repair_scored_weeks.sql` | Prune lineup rows appended after their week was scored; return a Triple Captain eaten by GW3; clear an ineligible GW1 score; backfill gameweek dates | **Executed 2026-08-24** |
| `2026-08-30_refresh_live_gameweek.sql` | Bring the live week's lineups up to the squads managers hold, so the deadline-snapshot change lands the same way for everyone | No marker — not run per this repo |
| `2026-08-30_repair_gameweek_5.sql` | Re-score gameweek 5, which was scored from squads frozen at the week's open rather than its deadline | No marker — not run per this repo |
| `2026-09-07_restore_confiscated_budget.sql` | Give back the budget the old carry-forward took from managers who did not spend it all | No marker — not run per this repo |

"Not run per this repo" means exactly that: the file carries no marker. It is
not proof the script was never run — only that nobody recorded it. Confirm
against production before acting on this table.

### Two of these have a window

`2026-08-30_refresh_live_gameweek.sql` is only safe **before the live week's
deadline**, and it was written for the deploy that shipped the deadline-snapshot
change (#54, merged). Its own precondition aborts it after the deadline.

`2026-08-30_repair_gameweek_5.sql` aborts if any transfer exists against a
gameweek later than 5. Gameweek 6's deadline has passed, so one transfer by any
manager closes it permanently. Before doing anything else with it, ask
production whether it can still run:

```sql
SELECT count(*) FROM transfers tr
JOIN match_weeks w ON w.id = tr.match_week_id
WHERE w.week_number > 5;
```

A non-zero answer means the script can no longer run, and gameweek 5's table
stays as scored. Record that in the file rather than weakening the guard — the
guard is what stops it doing damage with stale fingerprints.

## Known data questions with no script yet

**A double chip on gameweek 3.** One manager played a Triple Captain and a
Bench Boost on the same gameweek, back when nothing stopped them
(`backend/src/handlers/leagues.rs` records the incident; the scoreboard 500'd on
the pair). The write path now refuses a second chip per week, but the existing
row is still there.

No `UNIQUE(team_id, match_week_id)` migration was added, on purpose: migrations
run at boot, so one that fails on that row would stop the server rather than
raise the question. Reconcile the data first — if gameweek 3 is scored, the
second chip's points are already in `team_gameweek_points` and backing them out
means a re-score — then add the constraint.

```sql
SELECT tc.team_id, mw.week_number, array_agg(tc.chip_type)
FROM team_chips tc JOIN match_weeks mw ON mw.id = tc.match_week_id
GROUP BY tc.team_id, mw.week_number HAVING count(*) > 1;
```
