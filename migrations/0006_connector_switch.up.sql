-- Epic is the store that breaks. Its authentication has no public contract and
-- nothing stops it from changing again, so the connector needs a switch of its
-- own: turning it off has to leave Steam and GOG untouched.
--
-- The reason is kept and not only reported at the end of a run, because a
-- synchronisation that failed yesterday still explains the library the user is
-- looking at today. Without it, a store that stopped answering looks exactly
-- like a store with nothing new.
--
-- A store with no row here is on and with nothing wrong, so this table only
-- grows when something happens.
CREATE TABLE connector_state (
    store       TEXT PRIMARY KEY CHECK (store IN ('steam', 'gog', 'epic')),
    enabled     INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    last_error  TEXT,
    updated_at  TEXT NOT NULL
) STRICT;
