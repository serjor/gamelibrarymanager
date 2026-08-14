-- Para revisar un emparejamiento hay que poder mirar las dos caras: qué dice la
-- tienda y qué dice IGDB. Con solo los títulos no se distingue una ficha de otra
-- cuando IGDB repite entradas o cuando la tienda usa un nombre distinto.
--
-- Estas tres columnas son eso: la portada y la página de la copia en su tienda,
-- y el identificador con el que IGDB publica la suya. Ninguna la usa el
-- emparejamiento; son para que una persona compare.
ALTER TABLE store_entry ADD COLUMN cover_url TEXT;
ALTER TABLE store_entry ADD COLUMN store_url TEXT;
ALTER TABLE match_candidate ADD COLUMN slug TEXT;
