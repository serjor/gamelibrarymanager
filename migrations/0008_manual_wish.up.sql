-- A wish written by the user belongs to a game, not to a store account.
-- Synchronisation never writes here. A device has a family and a free model.
CREATE TABLE manual_wish (
    id          TEXT PRIMARY KEY,
    game_id     TEXT NOT NULL REFERENCES game (id) ON DELETE CASCADE,
    family      TEXT NOT NULL CHECK (family IN ('pc', 'playstation', 'xbox', 'nintendo', 'other')),
    model       TEXT NOT NULL,
    model_key   TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
) STRICT;

CREATE UNIQUE INDEX manual_wish_live_device
    ON manual_wish (game_id, family, model_key)
    WHERE deleted_at IS NULL;

CREATE INDEX manual_wish_live_game
    ON manual_wish (game_id) WHERE deleted_at IS NULL;
