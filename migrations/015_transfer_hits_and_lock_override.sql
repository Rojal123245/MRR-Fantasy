-- Allow multiple transfers per team in the same gameweek.
ALTER TABLE transfers
DROP CONSTRAINT IF EXISTS transfers_team_id_match_week_id_key;

-- Admin-controlled override for the scheduled weekend lineup lock.
CREATE TABLE IF NOT EXISTS lineup_lock_control (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),
    force_unlock BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO lineup_lock_control (id, force_unlock)
VALUES (TRUE, FALSE)
ON CONFLICT (id) DO NOTHING;
