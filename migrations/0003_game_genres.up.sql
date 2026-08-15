-- The genres come from IGDB and are kept as JSON in the record itself: they are
-- an attribute of the game, not an entity of their own, and no query must read
-- them in the opposite direction.
ALTER TABLE game ADD COLUMN genres TEXT NOT NULL DEFAULT '[]';
