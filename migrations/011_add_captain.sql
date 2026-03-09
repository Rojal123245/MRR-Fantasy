-- Add captain_id to fantasy_teams. The captain must be one of the 6 starters.
ALTER TABLE fantasy_teams ADD COLUMN captain_id UUID REFERENCES players(id) ON DELETE SET NULL;
