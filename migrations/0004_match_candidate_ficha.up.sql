-- La cola de revisión enseña candidatos que empatan muy a menudo: IGDB tiene
-- fichas duplicadas —seis «Limbo», dos «Hades»— y las ediciones de un mismo
-- juego se normalizan al mismo título. Con solo el nombre, esos empates son
-- indistinguibles y el usuario no puede decidir.
--
-- El año y la portada no los usa el emparejamiento: son para que una persona
-- separe de un vistazo lo que el algoritmo no se atreve a separar.
ALTER TABLE match_candidate ADD COLUMN release_year INTEGER;
ALTER TABLE match_candidate ADD COLUMN cover_url TEXT;
