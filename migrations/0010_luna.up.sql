-- What Amazon Luna includes with Prime. A game in this catalogue is a game the
-- user can play today without buying it, and that is the question a wish list
-- has to answer before a purchase.
--
-- Luna is not a store of the user here. It gives no copy, thus nothing goes in
-- `store_account`, `store_entry` or `connector_state`: a game that leaves the
-- catalogue at the end of the month must not leave the library with it.
--
-- One row, or none. With no row Luna is switched off and the application never
-- calls it. The country is what the user selected: it decides the headers of
-- the request and the domain of the links. The domain is kept here, beside the
-- country, so that the reading of the library builds a link with no knowledge
-- of the provider. The territory is what the answer of
-- Luna says, because Amazon selects the catalogue from the connection and not
-- from the request; the interface shows it so that a difference is visible.
-- Seconds from the epoch, as in `igdb_time_to_beat`: the interface compares it
-- to decide when to ask again.
CREATE TABLE luna_region (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    country       TEXT NOT NULL CHECK (length(country) = 2),
    site          TEXT NOT NULL,
    territory     TEXT,
    refreshed_at  INTEGER
) STRICT;

-- A cache of somebody else's data, as the prices and the durations: a refresh
-- deletes every row and writes the catalogue that just came in, in one
-- transaction. A game that left the catalogue and stayed marked would tell the
-- user not to buy something they cannot play.
--
-- The key is the identifier of Luna and not the ASIN: one game of the catalogue
-- of 2026-10-04, Fortnite, comes with no ASIN. There is no `game_id`. The
-- application matches a record to this catalogue by title when it reads, so a
-- wish added after the refresh is matched with no new request.
CREATE TABLE luna_catalog (
    product_id   TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    path         TEXT NOT NULL,
    asin         TEXT,
    captured_at  INTEGER NOT NULL
) STRICT;
