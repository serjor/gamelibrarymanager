import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type ReviewItem, type ScoredCandidate } from "../../lib/api";

/**
 * Base de las fichas públicas de IGDB. Se escribe como constante y se concatena
 * el slug, en vez de interpolar la dirección entera, porque el alcance de la
 * capacidad se comprueba contra las cadenas literales que hay en el código.
 */
const IGDB_GAME_URL = "https://www.igdb.com/games/";

/**
 * Lo que el emparejamiento automático no se atrevió a decidir.
 *
 * Que esta cola exista es la decisión de diseño central del producto: un
 * duplicado visible molesta, pero dos juegos distintos fusionados le hacen
 * perder al usuario el estado y las notas de uno de los dos, y encima sin
 * avisar. Ante la duda, se pregunta.
 *
 * Lo que casi siempre llega aquí no son dudas entre juegos distintos: son
 * empates entre fichas que **son el mismo juego** —IGDB tiene entradas
 * duplicadas, y las ediciones se normalizan al mismo título—. Por eso van
 * agrupados y se pueden resolver en lote: el umbral no se toca, porque acierta
 * al negarse cuando dos juegos distintos comparten nombre; lo que se arregla es
 * el trabajo de revisarlos.
 */
export function ReviewQueue({ items, onResolved }: { items: ReviewItem[]; onResolved: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Entrada -> ficha elegida, a la espera de confirmarse en lote. */
  const [elegidos, setElegidos] = useState<Record<string, number>>({});

  const [empates, sueltos] = useMemo(
    () => [items.filter((i) => i.tie), items.filter((i) => !i.tie)],
    [items],
  );

  const act = async (id: string, action: () => Promise<unknown>) => {
    setBusy(id);
    setError(null);
    try {
      await action();
      onResolved();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const confirmarLote = async () => {
    const decisiones = Object.entries(elegidos).map(
      ([entryId, igdbId]) => [entryId, igdbId] as [string, number],
    );
    if (decisiones.length === 0) return;
    await act("lote", async () => {
      await api.reviewConfirmMany(decisiones);
      setElegidos({});
    });
  };

  if (items.length === 0) {
    return <p className="hint">No hay nada pendiente de revisar.</p>;
  }

  const seleccionados = Object.keys(elegidos).length;

  const abrir = (url: string) => {
    openUrl(url).catch((cause: unknown) =>
      setError(`No he podido abrir ${url}: ${errorMessage(cause)}`),
    );
  };

  const fila = (item: ReviewItem) => (
    <li key={item.store_entry_id}>
      {/* Lo que dice la tienda, que es contra lo que hay que comparar. */}
      <div className="origen">
        {item.cover_url ? (
          <img src={item.cover_url} alt="" width={96} height={45} loading="lazy" />
        ) : (
          <span className="cover-missing wide" aria-hidden="true" />
        )}
        <p>
          <strong>{item.title}</strong>
          <br />
          <span className="hint">en {item.store}</span>
          {item.store_url && (
            <>
              {" · "}
              <button className="link" onClick={() => abrir(item.store_url!)}>
                ver en {item.store} ↗
              </button>
            </>
          )}
        </p>
      </div>

      {item.candidates.length === 0 ? (
        <p className="hint">IGDB no conoce este juego.</p>
      ) : (
        <>
          <p className="hint">Fichas de IGDB que podrían ser este juego:</p>
          <ul className="candidates">
            {item.candidates.map((candidate) => (
              <li key={candidate.igdb_id}>
                <Candidato
                  candidate={candidate}
                  elegido={elegidos[item.store_entry_id] === candidate.igdb_id}
                  onElegir={() =>
                    setElegidos((previos) =>
                      previos[item.store_entry_id] === candidate.igdb_id
                        ? Object.fromEntries(
                            Object.entries(previos).filter(([k]) => k !== item.store_entry_id),
                          )
                        : { ...previos, [item.store_entry_id]: candidate.igdb_id },
                    )
                  }
                  onMirar={
                    candidate.slug ? () => abrir(IGDB_GAME_URL + candidate.slug) : undefined
                  }
                />
              </li>
            ))}
          </ul>
        </>
      )}
      <button
        className="link"
        disabled={busy !== null}
        onClick={() =>
          void act(item.store_entry_id, () => api.reviewWithoutMetadata(item.store_entry_id))
        }
      >
        Ninguno: crear ficha con el título de la tienda
      </button>
    </li>
  );

  return (
    <section>
      <h2>Por revisar ({items.length})</h2>
      {error && <p role="alert">{error}</p>}

      {seleccionados > 0 && (
        <p className="hint sticky">
          <button disabled={busy !== null} onClick={() => void confirmarLote()}>
            {busy === "lote"
              ? "Confirmando…"
              : `Confirmar ${seleccionados} emparejamiento${seleccionados === 1 ? "" : "s"}`}
          </button>{" "}
          <button className="link" onClick={() => setElegidos({})}>
            deseleccionar
          </button>
        </p>
      )}

      {empates.length > 0 && (
        <>
          <h3>Empates ({empates.length})</h3>
          <p className="hint">
            Los mejores candidatos puntúan igual. Casi siempre son la misma ficha
            repetida en IGDB o ediciones del mismo juego, pero no siempre: dos
            juegos distintos pueden llamarse igual, y por eso no se decide solo.
            La portada y el año los distinguen.
          </p>
          <ul className="review">{empates.map(fila)}</ul>
        </>
      )}

      {sueltos.length > 0 && (
        <>
          {empates.length > 0 && <h3>El resto ({sueltos.length})</h3>}
          <ul className="review">{sueltos.map(fila)}</ul>
        </>
      )}
    </section>
  );
}

/** Un candidato con lo que hace falta para reconocerlo sin salir de la app. */
function Candidato({
  candidate,
  elegido,
  onElegir,
  onMirar,
}: {
  candidate: ScoredCandidate;
  elegido: boolean;
  onElegir: () => void;
  /** Abre su ficha en IGDB. Falta cuando IGDB no publicó un slug. */
  onMirar?: () => void;
}) {
  return (
    <span className="candidate-wrap">
      <button
        className={elegido ? "candidate chosen" : "candidate"}
        aria-pressed={elegido}
        onClick={onElegir}
      >
        {candidate.cover_url ? (
          // Decorativa: el nombre ya está en el propio botón, así que repetirlo
          // en el alt solo haría que un lector de pantalla lo dijese dos veces.
          <img src={candidate.cover_url} alt="" width={45} height={60} loading="lazy" />
        ) : (
          // Hueco del mismo tamaño. Sin él, un candidato sin portada se queda
          // como una pastilla baja al lado de una tarjeta alta y la fila deja de
          // leerse de un vistazo, que es justo para lo que están las portadas.
          <span className="cover-missing" aria-hidden="true" />
        )}
        <span>
          {candidate.name}
          {candidate.release_year !== null && (
            <span className="hint"> · {candidate.release_year}</span>
          )}
          <span className="hint"> · {Math.round(candidate.score * 100)}%</span>
        </span>
      </button>
      {onMirar && (
        // Botón aparte y no un enlace dentro del otro: anidar un control dentro
        // de un botón no es HTML válido y el teclado no llegaría al de dentro.
        <button
          className="link"
          onClick={onMirar}
          aria-label={`Ver ${candidate.name} en IGDB`}
        >
          IGDB ↗
        </button>
      )}
    </span>
  );
}
