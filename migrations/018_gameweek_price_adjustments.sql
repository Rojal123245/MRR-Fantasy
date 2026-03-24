-- Tracks per-gameweek price changes so admin can resubmit stats without double-counting.
CREATE TABLE gameweek_price_adjustments (
    match_week_id UUID NOT NULL REFERENCES match_weeks(id) ON DELETE CASCADE,
    player_id UUID NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    delta NUMERIC(10, 2) NOT NULL,
    PRIMARY KEY (match_week_id, player_id)
);

CREATE INDEX idx_gameweek_price_adjustments_week ON gameweek_price_adjustments(match_week_id);
