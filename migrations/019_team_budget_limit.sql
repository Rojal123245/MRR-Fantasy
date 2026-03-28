ALTER TABLE fantasy_teams
ADD COLUMN IF NOT EXISTS budget_limit NUMERIC(10, 2) NOT NULL DEFAULT 70.00;

-- Backfill existing teams so each user starts with their current squad value.
UPDATE fantasy_teams ft
SET budget_limit = team_cost.total_cost
FROM (
    SELECT tp.team_id, COALESCE(SUM(p.price), 0) AS total_cost
    FROM team_players tp
    JOIN players p ON p.id = tp.player_id
    GROUP BY tp.team_id
) AS team_cost
WHERE ft.id = team_cost.team_id;
