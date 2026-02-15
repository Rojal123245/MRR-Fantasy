-- Players table (real football players available for selection)
CREATE TYPE player_position AS ENUM ('GK', 'DEF', 'MID', 'FWD');

CREATE TABLE players (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(100) NOT NULL,
    position player_position NOT NULL,
    team_name VARCHAR(100) NOT NULL,
    photo_url VARCHAR(500),
    price DECIMAL(10, 2) NOT NULL DEFAULT 5.0,
    total_points INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_players_position ON players(position);
CREATE INDEX idx_players_team ON players(team_name);
