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
 *
 * De ahí la tabla. Una lista vertical obliga a leer entrada por entrada aunque
 * ciento cuarenta de las ciento cincuenta sean obvias; en columnas, lo que se
 * repasa es una sola —«se emparejará con»— y solo se baja al detalle donde algo
 * no cuadra.
 */
export function ReviewQueue({ items, onResolved }: { items: ReviewItem[]; onResolved: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /**
   * Lo que el usuario ha tocado: entrada -> ficha, o `null` si ha quitado la
   * que venía puesta. Lo que no está aquí es que no lo ha tocado, y entonces
   * manda la preselección. Guardar solo las diferencias evita el efecto que
   * copiaría la preselección a estado cada vez que la cola se recarga.
   */
  const [tocados, setTocados] = useState<Record<string, number | null>>({});

  const [empates, sueltos] = useMemo(
    () => [items.filter((i) => i.tie), items.filter((i) => !i.tie)],
    [items],
  );

  /**
   * Lo que no empata viene con el mejor candidato ya elegido; lo que empata,
   * con nada.
   *
   * Esa asimetría es la cola entera. Cuando un candidato gana con holgura, lo
   * que queda por hacer es mirar si es él y decir que sí, y hacer clic para
   * repetir lo que la pantalla ya dice es trabajo inventado. Cuando dos
   * empatan, elegir por el usuario sería justo lo que el umbral se negó a
   * hacer, y el motivo por el que existe esta pantalla.
   */
  const preseleccion = useMemo(
    () =>
      Object.fromEntries(
        sueltos
          .filter((item) => item.candidates.length > 0)
          .map((item) => [item.store_entry_id, item.candidates[0]!.igdb_id]),
      ) as Record<string, number>,
    [sueltos],
  );

  const elegido = (item: ReviewItem): number | null => {
    const tocado = tocados[item.store_entry_id];
    return tocado === undefined ? (preseleccion[item.store_entry_id] ?? null) : tocado;
  };

  // Pulsar la que ya está puesta la quita: es la única forma de decir «esta no»
  // sin decir a la vez cuál sí, y hace falta para dejar una entrada fuera del
  // lote sin resolverla.
  const elegir = (item: ReviewItem, igdbId: number) => {
    const actual = elegido(item);
    setTocados((previos) => ({
      ...previos,
      [item.store_entry_id]: actual === igdbId ? null : igdbId,
    }));
  };

  const decisiones = items
    .map((item) => [item.store_entry_id, elegido(item)] as const)
    .filter((par): par is [string, number] => par[1] !== null);

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
    if (decisiones.length === 0) return;
    await act("lote", async () => {
      await api.reviewConfirmMany(decisiones.map(([entrada, ficha]) => [entrada, ficha]));
      setTocados({});
    });
  };

  if (items.length === 0) {
    return <p className="hint">No hay nada pendiente de revisar.</p>;
  }

  const abrir = (url: string) => {
    openUrl(url).catch((cause: unknown) =>
      setError(`No he podido abrir ${url}: ${errorMessage(cause)}`),
    );
  };

  const fila = (item: ReviewItem) => {
    const id = elegido(item);
    const puesto = item.candidates.find((candidate) => candidate.igdb_id === id) ?? null;
    const otros = item.candidates.filter((candidate) => candidate.igdb_id !== id);

    return (
      <tr key={item.store_entry_id} className={puesto === null ? "sin-elegir" : undefined}>
        {/* Lo que dice la tienda, que es contra lo que hay que comparar. */}
        <td>
          <div className="origen">
            {item.cover_url ? (
              <img src={item.cover_url} alt="" width={96} height={45} loading="lazy" />
            ) : (
              <span className="cover-missing wide" aria-hidden="true" />
            )}
            <strong>{item.title}</strong>
          </div>
        </td>

        <td>
          <span className="tienda">{item.store}</span>
          {item.store_url && (
            <button
              className="link"
              onClick={() => abrir(item.store_url!)}
              aria-label={`Ver ${item.title} en ${item.store}`}
            >
              ↗
            </button>
          )}
        </td>

        <td>
          {puesto ? (
            <Candidato
              candidate={puesto}
              elegido
              resumido
              onElegir={() => elegir(item, puesto.igdb_id)}
              onMirar={puesto.slug ? () => abrir(IGDB_GAME_URL + puesto.slug) : undefined}
            />
          ) : (
            <span className="hint">sin elegir</span>
          )}
        </td>

        <td className="num">{puesto?.release_year ?? "—"}</td>
        <td className="num">{puesto ? `${Math.round(puesto.score * 100)}%` : "—"}</td>

        <td>
          {item.candidates.length === 0 ? (
            <p className="hint">IGDB no conoce este juego.</p>
          ) : (
            <ul className="candidates">
              {otros.map((candidate) => (
                <li key={candidate.igdb_id}>
                  <Candidato
                    candidate={candidate}
                    elegido={false}
                    onElegir={() => elegir(item, candidate.igdb_id)}
                    onMirar={
                      candidate.slug ? () => abrir(IGDB_GAME_URL + candidate.slug) : undefined
                    }
                  />
                </li>
              ))}
            </ul>
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
        </td>
      </tr>
    );
  };

  const tabla = (filas: ReviewItem[]) => (
    <div className="revision-viewport">
      <table className="revision">
        <colgroup>
          <col />
          <col style={{ width: "5.5rem" }} />
          <col style={{ width: "17rem" }} />
          <col style={{ width: "4rem" }} />
          <col style={{ width: "5.5rem" }} />
          <col style={{ width: "20rem" }} />
        </colgroup>
        <thead>
          <tr>
            <th>En la tienda</th>
            <th>Tienda</th>
            <th>Se emparejará con</th>
            <th className="num">Año</th>
            <th className="num">Parecido</th>
            <th>Otras fichas de IGDB</th>
          </tr>
        </thead>
        <tbody>{filas.map(fila)}</tbody>
      </table>
    </div>
  );

  return (
    <section className="revision-pantalla">
      <h2>Por revisar ({items.length})</h2>
      {error && <p role="alert">{error}</p>}

      {decisiones.length > 0 && (
        <p className="hint sticky">
          <button disabled={busy !== null} onClick={() => void confirmarLote()}>
            {busy === "lote"
              ? "Confirmando…"
              : `Confirmar ${decisiones.length} emparejamiento${decisiones.length === 1 ? "" : "s"}`}
          </button>{" "}
          <button
            className="link"
            onClick={() =>
              setTocados(Object.fromEntries(items.map((item) => [item.store_entry_id, null])))
            }
          >
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
          {tabla(empates)}
        </>
      )}

      {sueltos.length > 0 && (
        <>
          {empates.length > 0 && <h3>El resto ({sueltos.length})</h3>}
          {/* Decir qué va a pasar al pulsar el botón del lote. Una preselección
              callada es la manera de que alguien confirme ciento cincuenta
              emparejamientos creyendo que confirmaba los que había tocado. */}
          <p className="hint">
            Aquí un candidato gana con holgura, así que viene ya elegido. Repasa
            la columna «se emparejará con» y cambia lo que no cuadre: nada se
            escribe hasta que confirmes.
          </p>
          {tabla(sueltos)}
        </>
      )}
    </section>
  );
}

/** Un candidato con lo que hace falta para reconocerlo sin salir de la app. */
function Candidato({
  candidate,
  elegido,
  resumido,
  onElegir,
  onMirar,
}: {
  candidate: ScoredCandidate;
  elegido: boolean;
  /** En la columna de elegido el año y el parecido tienen la suya, y repetirlos ahí es ruido. */
  resumido?: boolean;
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
          {!resumido && candidate.release_year !== null && (
            <span className="hint"> · {candidate.release_year}</span>
          )}
          {!resumido && <span className="hint"> · {Math.round(candidate.score * 100)}%</span>}
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
