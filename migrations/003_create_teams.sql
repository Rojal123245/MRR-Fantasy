-- Fantasy teams (each user can have one team of 6 players)
CREATE TABLE fantasy_teams (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id)
);

CREATE TABLE team_players (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    team_id UUID NOT NULL REFERENCES fantasy_teams(id) ON DELETE CASCADE,
    player_id UUID NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    UNIQUE(team_id, player_id)
);

-- Enforce max 6 players per team via a function + trigger
CREATE OR REPLACE FUNCTION check_team_player_limit()
RETURNS TRIGGER AS $$
BEGIN
    IF (SELECT COUNT(*) FROM team_players WHERE team_id = NEW.team_id) >= 6 THEN
        RAISE EXCEPTION 'A fantasy team cannot have more than 6 players';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER enforce_team_player_limit
    BEFORE INSERT ON team_players
    FOR EACH ROW
    EXECUTE FUNCTION check_team_player_limit();

CREATE INDEX idx_fantasy_teams_user ON fantasy_teams(user_id);
CREATE INDEX idx_team_players_team ON team_players(team_id);
CREATE INDEX idx_team_players_player ON team_players(player_id);
