CREATE TABLE futsal_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    total_amount NUMERIC(10, 2) NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE futsal_session_players (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES futsal_sessions(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id),
    player_name TEXT NOT NULL,
    amount_due NUMERIC(10, 2) NOT NULL DEFAULT 0,
    is_paid BOOLEAN NOT NULL DEFAULT FALSE,
    paid_at TIMESTAMPTZ,
    marked_paid_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fsp_session ON futsal_session_players(session_id);
CREATE INDEX idx_fsp_user ON futsal_session_players(user_id);
