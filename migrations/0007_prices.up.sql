-- Prices turn a wish list into a buying decision. They come from IsThereAnyDeal,
-- which is a fifth layer on top of the four the schema already has: not what the
-- store says, not what the application deduces, not the record, and not what the
-- user writes.
--
-- Two identifiers and not one. The UUID is what the batch price endpoint takes,
-- and the slug is what builds the page of the game on ITAD. That page is the
-- only address the interface opens, because a deal points at whatever shop sells
-- it and the capability of the window cannot allow a host it does not know.
--
-- A record with no identifier is a record ITAD does not know yet, or one the
-- lookup by title did not settle. The next refresh asks again: a wish list is
-- small enough for that, and a game that ITAD adds tomorrow appears by itself.
ALTER TABLE game ADD COLUMN itad_id TEXT;
ALTER TABLE game ADD COLUMN itad_slug TEXT;

-- Money in cents, and never as a real number. A price is a count of the smallest
-- unit of its currency, and floating point turns 19.99 into 19.989999 the first
-- time somebody adds two of them.
--
-- These two tables are a cache of somebody else's data, and the only ones in the
-- schema whose rows are deleted for real. The rule that forbids it protects what
-- the user cannot get back —a copy that left a store keeps its state and its
-- notes— and a price is the opposite: it belongs to a shop, it changes by the
-- hour and the next refresh brings it again. A soft deleted price is worse than
-- no price, because an offer that ended still looks like an offer.
CREATE TABLE price_snapshot (
    game_id      TEXT NOT NULL REFERENCES game (id) ON DELETE CASCADE,
    -- The name of the shop as ITAD publishes it, and not a `store` of this
    -- schema. ITAD knows dozens of shops this application does not connect to,
    -- and the best price of a wished game very often lives in one of them.
    shop         TEXT NOT NULL,
    amount       INTEGER NOT NULL,
    regular      INTEGER NOT NULL,
    cut          INTEGER NOT NULL,
    currency     TEXT NOT NULL,
    captured_at  TEXT NOT NULL,
    PRIMARY KEY (game_id, shop)
) STRICT;

-- The historical low is a fact of the game and not of the shop: ITAD computes it
-- across every shop it watches and publishes it once. Repeating it on each deal
-- would let two rows of the same game disagree, and then reading it would depend
-- on which row the query happened to pick.
CREATE TABLE price_low (
    game_id      TEXT PRIMARY KEY REFERENCES game (id) ON DELETE CASCADE,
    all_time     INTEGER,
    year         INTEGER,
    currency     TEXT NOT NULL,
    captured_at  TEXT NOT NULL
) STRICT;
