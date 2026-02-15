-- Add secondary position column (a player can play max 2 positions)
ALTER TABLE players ADD COLUMN IF NOT EXISTS secondary_position player_position;

CREATE INDEX idx_players_secondary_position ON players(secondary_position);
