-- Add detailed match stat columns to player_points
ALTER TABLE player_points ADD COLUMN own_goals INTEGER NOT NULL DEFAULT 0;
ALTER TABLE player_points ADD COLUMN penalty_misses INTEGER NOT NULL DEFAULT 0;
ALTER TABLE player_points ADD COLUMN penalty_saves INTEGER NOT NULL DEFAULT 0;
ALTER TABLE player_points ADD COLUMN regular_fouls INTEGER NOT NULL DEFAULT 0;
ALTER TABLE player_points ADD COLUMN serious_fouls INTEGER NOT NULL DEFAULT 0;
ALTER TABLE player_points ADD COLUMN minutes_played INTEGER NOT NULL DEFAULT 0;

-- Add admin flag to users
ALTER TABLE users ADD COLUMN is_admin BOOLEAN NOT NULL DEFAULT FALSE;
