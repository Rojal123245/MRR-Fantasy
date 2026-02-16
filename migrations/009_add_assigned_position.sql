-- Add assigned_position column to track what role each starter plays.
-- NULL for bench players; enforced as non-null for starters in application code.
ALTER TABLE team_players ADD COLUMN assigned_position player_position;
