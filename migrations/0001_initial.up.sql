-- Cuatro capas separadas a propósito:
--   store_entry  lo que dice la tienda      (lo escribe la sincronización)
--   game_link    lo que deduce la app       (lo reescribe el emparejamiento)
--   game         la ficha canónica          (metadatos)
--   user_state   lo que escribe el usuario  (no lo toca nada más)
--
-- Ninguna tabla borra filas: `deleted_at` marca la baja y conserva el histórico,
-- que es lo que permitirá sincronizar entre dispositivos sin resucitar registros.

CREATE TABLE store_account (
    id            TEXT PRIMARY KEY,
    store         TEXT NOT NULL CHECK (store IN ('steam', 'gog', 'epic')),
    account_ref   TEXT NOT NULL,
    display_name  TEXT,
    connected_at  TEXT NOT NULL,
    last_sync_at  TEXT,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT,
    UNIQUE (store, account_ref)
) STRICT;

CREATE TABLE store_entry (
    id                TEXT PRIMARY KEY,
    account_id        TEXT NOT NULL REFERENCES store_account (id) ON DELETE CASCADE,
    store             TEXT NOT NULL CHECK (store IN ('steam', 'gog', 'epic')),
    store_app_id      TEXT NOT NULL,
    kind              TEXT NOT NULL CHECK (kind IN ('owned', 'wishlist')),
    title             TEXT NOT NULL,
    playtime_minutes  INTEGER,
    acquired_at       TEXT,
    raw               TEXT NOT NULL,
    first_seen_at     TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    deleted_at        TEXT,
    UNIQUE (account_id, store_app_id, kind)
) STRICT;

CREATE INDEX store_entry_by_kind ON store_entry (kind, deleted_at);

CREATE TABLE game (
    id               TEXT PRIMARY KEY,
    canonical_title  TEXT NOT NULL,
    sort_title       TEXT NOT NULL,
    igdb_id          INTEGER UNIQUE,
    cover_url        TEXT,
    summary          TEXT,
    released_at      TEXT,
    updated_at       TEXT NOT NULL,
    deleted_at       TEXT
) STRICT;

CREATE INDEX game_by_sort_title ON game (sort_title);

-- Una entrada de tienda pertenece como mucho a un juego: el índice único sobre
-- store_entry_id es lo que impide que un emparejamiento defectuoso duplique la
-- misma copia en dos fichas.
CREATE TABLE game_link (
    game_id         TEXT NOT NULL REFERENCES game (id) ON DELETE CASCADE,
    store_entry_id  TEXT NOT NULL REFERENCES store_entry (id) ON DELETE CASCADE,
    confidence      REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    method          TEXT NOT NULL CHECK (method IN ('auto', 'manual')),
    updated_at      TEXT NOT NULL,
    PRIMARY KEY (game_id, store_entry_id)
) STRICT;

CREATE UNIQUE INDEX game_link_one_game_per_entry ON game_link (store_entry_id);

CREATE TABLE user_state (
    game_id      TEXT PRIMARY KEY REFERENCES game (id) ON DELETE CASCADE,
    status       TEXT CHECK (status IN ('backlog', 'playing', 'finished', 'abandoned')),
    rating       INTEGER CHECK (rating BETWEEN 1 AND 10),
    notes        TEXT,
    started_at   TEXT,
    finished_at  TEXT,
    updated_at   TEXT NOT NULL
) STRICT;
