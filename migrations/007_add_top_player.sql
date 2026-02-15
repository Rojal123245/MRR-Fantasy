-- Add top player flag for premium players (max 2 per team)
ALTER TABLE players ADD COLUMN IF NOT EXISTS is_top_player BOOLEAN NOT NULL DEFAULT false;
