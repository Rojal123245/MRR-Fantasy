-- Add bench support: 6 starters + 3 bench (1 GK + 2 outfield)
ALTER TABLE team_players ADD COLUMN IF NOT EXISTS is_bench BOOLEAN NOT NULL DEFAULT false;

-- Update trigger to allow 9 players total
CREATE OR REPLACE FUNCTION check_team_player_limit()
RETURNS TRIGGER AS $$
BEGIN
    IF (SELECT COUNT(*) FROM team_players WHERE team_id = NEW.team_id) >= 9 THEN
        RAISE EXCEPTION 'A fantasy team cannot have more than 9 players (6 starters + 3 bench)';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
