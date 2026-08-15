-- To examine a match you must see the two sides: what the store says and what
-- IGDB says. With the titles alone you cannot tell one record from another when
-- IGDB repeats entries or when the store uses a different name.
--
-- These three columns are that: the cover and the page of the copy in its store,
-- and the identifier with which IGDB publishes its own record. The matching uses
-- none of them; they exist so that a person can compare.
ALTER TABLE store_entry ADD COLUMN cover_url TEXT;
ALTER TABLE store_entry ADD COLUMN store_url TEXT;
ALTER TABLE match_candidate ADD COLUMN slug TEXT;
