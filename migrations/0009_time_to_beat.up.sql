-- How long a game takes, as the players report it to IGDB. It is the answer to
-- "can I finish this before the weekend", which the backlog asks and the store
-- does not.
--
-- The key is the IGDB record and not the `game` row. This table is a copy of a
-- table of IGDB, and its key is the key of IGDB: two rows of `game` cannot carry
-- the same `igdb_id`, and a record that loses its IGDB identity stops finding
-- its durations with no extra step.
--
-- A cache of somebody else's data, as the prices: the next pass replaces a row
-- complete and nothing is soft deleted. What the user wrote never lives here.
--
-- A row with no duration is an answer too: IGDB knows the record and nobody
-- reported a time. It is kept so that the next pass does not ask again at once,
-- and `checked_at` says when to ask again. Seconds from the epoch, and not text:
-- the pass compares it, and a text date with fractions of a second of variable
-- length does not sort as the moment that it names.
CREATE TABLE igdb_time_to_beat (
    igdb_id      INTEGER PRIMARY KEY,
    hastily      INTEGER CHECK (hastily > 0),
    normally     INTEGER CHECK (normally > 0),
    completely   INTEGER CHECK (completely > 0),
    submissions  INTEGER NOT NULL CHECK (submissions >= 0),
    checked_at   INTEGER NOT NULL
) STRICT;
