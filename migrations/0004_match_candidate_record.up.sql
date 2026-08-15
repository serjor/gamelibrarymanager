-- The review queue shows candidates that are equal very frequently: IGDB has
-- duplicate records — six "Limbo", two "Hades" — and the editions of one game
-- normalise to the same title. With the name alone, you cannot tell those equal
-- candidates apart and the user cannot decide.
--
-- The matching does not use the year and the cover: they exist so that a person
-- can quickly tell apart what the algorithm does not separate.
ALTER TABLE match_candidate ADD COLUMN release_year INTEGER;
ALTER TABLE match_candidate ADD COLUMN cover_url TEXT;
