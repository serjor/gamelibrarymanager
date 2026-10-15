-- The games that the Steam family of the user can lend. A game of the family is
-- a game the user can play today without buying it, and that is the question a
-- wish list has to answer before a purchase.
--
-- The family is not a store of the user here. The games belong to other
-- persons, thus nothing goes in `store_account`, `store_entry` or
-- `connector_state`: a game whose owner leaves the family must not leave the
-- library with them.
--
-- One row, or none. With no row the family is switched off and the
-- synchronisation never asks Steam for it. `family_groupid` is NULL when Steam
-- says that the user is in no family, and it is text: Steam gives a 64-bit
-- number. `last_error` is the reason of the last stage that did not end, so
-- that Utilities can say why the list is old. Seconds from the epoch, as in
-- `luna_region`.
CREATE TABLE steam_family (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    family_groupid  TEXT,
    refreshed_at    INTEGER,
    last_error      TEXT
) STRICT;

-- A cache of the data of other persons, as Luna: each synchronisation deletes
-- every row and writes the list that just came in, in one transaction. A game
-- that left the family and stayed marked would tell the user not to buy
-- something they cannot play. It is the fifth table of the closed list of
-- `docs/storage/prices-are-a-cache-that-is-replaced.md`.
--
-- There is no `game_id`. The application matches a record to this list when
-- it reads: by the Steam appid of a copy, by `igdb_id`, or by title. `igdb_id`
-- is written by the identity pass and not by the synchronisation, thus the
-- replacement keeps it for the appids that stay. `owners` is a JSON list of
-- `{"steam_id", "name"}`: the names change, and they come again at each
-- synchronisation with the rest of the row.
CREATE TABLE steam_family_app (
    app_id       TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    owners       TEXT NOT NULL,
    igdb_id      INTEGER,
    captured_at  INTEGER NOT NULL
) STRICT;
