import { useMemo, useState } from "react";
import type { LibraryRow } from "../../lib/api";
import { ETIQUETA_ESTADO } from "../../lib/estado";
import { GameDetail } from "../game/GameDetail";
import { ANCHO, ALTO_PORTADA } from "../library/LibraryWall";
import { destacado, estanterias } from "./shelves";

/**
 * A qué jugar hoy.
 *
 * No es un tercer modo de vista de la biblioteca, y por eso no lee sus filtros:
 * la tabla y la pared comparten contrato —filtras y las dos enseñan lo
 * filtrado—, mientras que esta pantalla hace sus propios cortes. Sobre
 * estanterías curadas, «género = RPG» no significaría nada.
 *
 * La ficha se abre siempre como hoja: aquí no hay una lista al lado que
 * mantener a la vista, y lo que se está mirando es el arte.
 */
export function Today({ rows, onSaved }: { rows: LibraryRow[]; onSaved: () => void }) {
  const [abierto, setAbierto] = useState<string | null>(null);
  // Congelado al montar: las estanterías cortan por «hace seis meses», y un
  // reloj que avanza a mitad de render haría que dos cálculos de la misma
  // pantalla no coincidieran.
  const [ahora] = useState(() => Math.floor(Date.now() / 1000));

  const propuesta = useMemo(() => destacado(rows, ahora), [rows, ahora]);
  // El destacado no se repite abajo: verlo dos veces en la misma pantalla hace
  // pensar que son dos juegos. Si por eso una estantería se queda vacía, no se
  // pinta, que es lo que ya hace con cualquier otra vacía.
  const estantes = useMemo(
    () => estanterias(rows.filter((row) => row.game_id !== propuesta?.juego.game_id), ahora),
    [rows, propuesta, ahora],
  );
  const abiertoRow = useMemo(
    () => rows.find((row) => row.game_id === abierto) ?? null,
    [rows, abierto],
  );

  if (propuesta === null) {
    return (
      <p className="hint">
        Todavía no hay ningún juego en propiedad que proponer. Sincroniza una
        tienda y aquí aparecerá a qué jugar.
      </p>
    );
  }

  const juego = propuesta.juego;

  return (
    <section className="hoy">
      <article className="destacado">
        <div className="destacado-arte">
          {juego.cover_url ? (
            <img src={juego.cover_url} alt="" />
          ) : (
            // Decorativa: el título está justo al lado, y repetirlo obliga a un
            // lector de pantalla a decirlo dos veces.
            <span className="cover-placeholder" aria-hidden="true">
              {juego.title}
            </span>
          )}
        </div>

        <div className="destacado-texto">
          <p className="hint">{propuesta.motivo}</p>
          <h2>{juego.title}</h2>
          <p className="hint">
            {juego.release_year ?? "año desconocido"}
            {juego.genres.length > 0 && ` · ${juego.genres.join(", ")}`}
            {` · ${juego.owned_stores.join(", ")}`}
            {juego.status && ` · ${ETIQUETA_ESTADO[juego.status]}`}
          </p>
          {juego.summary && <p className="resumen">{juego.summary}</p>}
          <div className="actions">
            <button onClick={() => setAbierto(juego.game_id)}>Abrir la ficha</button>
          </div>
        </div>
      </article>

      {estantes.map((estante) => (
        <section key={estante.id} className="estante-caja">
          <h3>{estante.titulo}</h3>
          <p className="hint">{estante.motivo}</p>
          <ul
            className="estante"
            style={{
              gridAutoColumns: `${ANCHO}px`,
              ["--alto-portada" as string]: `${ALTO_PORTADA}px`,
            }}
          >
            {estante.juegos.map((row) => (
              <li key={row.game_id}>
                <button className="baldosa" onClick={() => setAbierto(row.game_id)}>
                  {row.cover_url ? (
                    <img src={row.cover_url} alt="" loading="lazy" />
                  ) : (
                    <span className="cover-placeholder" aria-hidden="true">
                      {row.title}
                    </span>
                  )}
                  <span className="baldosa-titulo">{row.title}</span>
                  <span className="hint">{row.owned_stores.join(" · ")}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}

      {abiertoRow && (
        <GameDetail
          key={abiertoRow.game_id}
          row={abiertoRow}
          variant="sheet"
          onClose={() => setAbierto(null)}
          onSaved={onSaved}
        />
      )}
    </section>
  );
}
