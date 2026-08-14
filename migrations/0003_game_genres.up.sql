-- Los géneros vienen de IGDB y se guardan como JSON en la propia ficha: son un
-- atributo del juego, no una entidad con vida propia, y no hay ninguna consulta
-- que necesite recorrerlos al revés.
ALTER TABLE game ADD COLUMN genres TEXT NOT NULL DEFAULT '[]';
