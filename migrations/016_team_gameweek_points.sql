CREATE TABLE IF NOT EXISTS team_gameweek_points (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES fantasy_teams(id) ON DELETE CASCADE,
    match_week_id UUID NOT NULL REFERENCES match_weeks(id) ON DELETE CASCADE,
    gross_points INTEGER NOT NULL DEFAULT 0,
    transfer_points_hit INTEGER NOT NULL DEFAULT 0,
    total_points INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(team_id, match_week_id)
);

CREATE INDEX IF NOT EXISTS idx_team_gameweek_points_team
    ON team_gameweek_points(team_id);

CREATE INDEX IF NOT EXISTS idx_team_gameweek_points_week
    ON team_gameweek_points(match_week_id);
