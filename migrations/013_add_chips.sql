-- Chips: Triple Captain and Bench Boost (each usable once per team)
CREATE TABLE team_chips (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    team_id UUID NOT NULL REFERENCES fantasy_teams(id) ON DELETE CASCADE,
    chip_type TEXT NOT NULL CHECK (chip_type IN ('triple_captain', 'bench_boost')),
    match_week_id UUID NOT NULL REFERENCES match_weeks(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(team_id, chip_type)
);

CREATE INDEX idx_team_chips_team ON team_chips(team_id);
CREATE INDEX idx_team_chips_type ON team_chips(chip_type);
