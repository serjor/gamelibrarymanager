-- La cola de revisión necesita recordar qué encontró el emparejamiento, o cada
-- vez que el usuario abriera la pantalla habría que volver a preguntar a IGDB
-- y gastar cuota en algo que ya se sabía.
--
-- Es caché, no verdad: se puede borrar entera y reconstruir sin perder nada.
CREATE TABLE match_candidate (
    store_entry_id  TEXT NOT NULL REFERENCES store_entry (id) ON DELETE CASCADE,
    igdb_id         INTEGER NOT NULL,
    name            TEXT NOT NULL,
    score           REAL NOT NULL,
    updated_at      TEXT NOT NULL,
    PRIMARY KEY (store_entry_id, igdb_id)
) STRICT;

CREATE INDEX match_candidate_by_score ON match_candidate (store_entry_id, score DESC);
