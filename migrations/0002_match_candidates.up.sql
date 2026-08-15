-- The review queue must remember what the matching found. Without this table,
-- each time that the user opened the screen the application would ask IGDB again
-- and use quota for data that it already had.
--
-- This is a cache, not the truth: you can delete all of it and build it again
-- and lose nothing.
CREATE TABLE match_candidate (
    store_entry_id  TEXT NOT NULL REFERENCES store_entry (id) ON DELETE CASCADE,
    igdb_id         INTEGER NOT NULL,
    name            TEXT NOT NULL,
    score           REAL NOT NULL,
    updated_at      TEXT NOT NULL,
    PRIMARY KEY (store_entry_id, igdb_id)
) STRICT;

CREATE INDEX match_candidate_by_score ON match_candidate (store_entry_id, score DESC);
