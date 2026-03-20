CREATE TABLE IF NOT EXISTS team_gameweek_lineups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES fantasy_teams(id) ON DELETE CASCADE,
    match_week_id UUID NOT NULL REFERENCES match_weeks(id) ON DELETE CASCADE,
    captain_id UUID REFERENCES players(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(team_id, match_week_id)
);

CREATE INDEX IF NOT EXISTS idx_team_gameweek_lineups_team
    ON team_gameweek_lineups(team_id);

CREATE INDEX IF NOT EXISTS idx_team_gameweek_lineups_week
    ON team_gameweek_lineups(match_week_id);

CREATE TABLE IF NOT EXISTS team_gameweek_lineup_players (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_gameweek_lineup_id UUID NOT NULL REFERENCES team_gameweek_lineups(id) ON DELETE CASCADE,
    player_id UUID NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    is_bench BOOLEAN NOT NULL DEFAULT FALSE,
    assigned_position player_position,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(team_gameweek_lineup_id, player_id)
);

CREATE INDEX IF NOT EXISTS idx_team_gameweek_lineup_players_lineup
    ON team_gameweek_lineup_players(team_gameweek_lineup_id);
