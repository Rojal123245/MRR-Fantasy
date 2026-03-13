CREATE TABLE IF NOT EXISTS transfers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES fantasy_teams(id),
    match_week_id UUID NOT NULL REFERENCES match_weeks(id),
    player_out_id UUID NOT NULL REFERENCES players(id),
    player_in_id UUID NOT NULL REFERENCES players(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(team_id, match_week_id)
);
