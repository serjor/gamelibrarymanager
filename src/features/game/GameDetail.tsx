import { useEffect, useRef, useState } from "react";
import { api, errorMessage, type LibraryRow, type PlayStatus } from "../../lib/api";
import { ESTADOS, ETIQUETA_ESTADO } from "../../lib/estado";

/**
 * Acoplada al lado de la tabla, o superpuesta sobre las portadas.
 *
 * No son dos fichas: es la misma con dos maneras de enseñarse. Lo que cambia es
 * el envoltorio y el arte; el formulario, el guardado y la validación son los
 * mismos objetos en el mismo sitio.
 */
export type Presentacion = "inspector" | "sheet";

function horas(minutos: number): string {
  if (minutos === 0) return "sin jugar";
  if (minutos < 60) return `${minutos} min`;
  return `${Math.round(minutos / 60)} h`;
}

/**
 * La ficha unificada: los metadatos, en qué tiendas está la copia, y lo único
 * que el usuario escribe. Guardar es explícito para que no se pierda una nota a
 * medio escribir al cerrar el panel.
 *
 * El estado del formulario se inicializa una vez y no se sincroniza con las
 * props: quien lo llama pasa `key={row.game_id}`, así que cambiar de juego
 * remonta el panel y no hace falta ningún efecto que copie props a estado.
 *
 * Solo hay una llamada a `api.setUserState` en todo el fichero, y eso es la
 * comprobación de que las dos presentaciones guardan por el mismo camino: no
 * hay un segundo sitio donde una pueda empezar a validar distinto que la otra.
 */
export function GameDetail({
  row,
  variant,
  onClose,
  onSaved,
}: {
  row: LibraryRow;
  variant: Presentacion;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [status, setStatus] = useState<PlayStatus | null>(row.status);
  const [rating, setRating] = useState<number | null>(row.rating);
  const [notes, setNotes] = useState(row.notes ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hoja = useRef<HTMLDialogElement | null>(null);

  // Modal de verdad y no un `div` con `role="dialog"`: atrapar el foco, cerrar
  // con Escape y devolver el foco a donde estaba lo hace ya el navegador, y
  // hecho a mano son cien líneas que se equivocan en los casos raros.
  useEffect(() => {
    hoja.current?.showModal();
  }, []);

  // Cerrar por el camino del navegador cuando hay diálogo, que es lo que
  // devuelve el foco a la baldosa desde la que se abrió.
  const cerrar = () => (hoja.current === null ? onClose() : hoja.current.close());

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.setUserState(row.game_id, status, rating, notes.trim() || null);
      onSaved();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  const ficha = (
    <>
      <header>
        <h2 id="ficha-titulo">{row.title}</h2>
        <button className="link" onClick={cerrar} aria-label="Cerrar ficha">
          cerrar
        </button>
      </header>

      <p className="hint">
        {row.release_year ?? "año desconocido"} · {horas(row.playtime_minutes)}
        {row.genres.length > 0 && ` · ${row.genres.join(", ")}`}
      </p>

      <p className="hint">
        {row.owned_stores.length > 0
          ? `En propiedad: ${row.owned_stores.join(", ")}`
          : "No lo tienes en ninguna tienda"}
        {row.wishlist_stores.length > 0 && ` · Deseado en: ${row.wishlist_stores.join(", ")}`}
      </p>

      {/* El resumen es de IGDB, así que falta justo en las fichas que nacieron
          del título de la tienda. Decirlo vale más que dejar un hueco mudo: es
          la misma promesa que hace el aviso de la cabecera. */}
      {row.summary ? (
        <p className="resumen">{row.summary}</p>
      ) : (
        <p className="hint">Sin resumen: la ficha se creó con el título de la tienda.</p>
      )}

      <label htmlFor="status">Estado</label>
      <select
        id="status"
        value={status ?? ""}
        onChange={(e) => setStatus((e.target.value || null) as PlayStatus | null)}
      >
        <option value="">Sin marcar</option>
        {ESTADOS.map((valor) => (
          <option key={valor} value={valor}>
            {ETIQUETA_ESTADO[valor]}
          </option>
        ))}
      </select>

      <label htmlFor="rating">Nota (1-10)</label>
      <input
        id="rating"
        type="number"
        min={1}
        max={10}
        value={rating ?? ""}
        onChange={(e) => setRating(e.target.value === "" ? null : Number(e.target.value))}
      />

      <label htmlFor="notes">Notas</label>
      <textarea id="notes" rows={4} value={notes} onChange={(e) => setNotes(e.target.value)} />

      {error && <p role="alert">{error}</p>}

      <button onClick={() => void save()} disabled={busy}>
        {busy ? "Guardando…" : "Guardar"}
      </button>
    </>
  );

  if (variant === "inspector") {
    return <aside className="detail ficha">{ficha}</aside>;
  }

  return (
    <dialog
      className="hoja"
      ref={hoja}
      aria-labelledby="ficha-titulo"
      onClose={onClose}
      // El velo lo pinta el propio diálogo, así que un clic fuera de la caja
      // llega aquí con el diálogo de destino y no hay que medir coordenadas.
      onClick={(evento) => {
        if (evento.target === hoja.current) cerrar();
      }}
    >
      <div className="hoja-caja">
        {/* Apaisada y de la tienda, que es lo que la hoja tiene y el inspector
            no: la cabecera de Steam o el logo de GOG, recortados a la misma
            caja para que la ficha empiece siempre a la misma altura.
            Decorativa: el título va justo debajo. */}
        {row.store_cover_url ? (
          <img className="hoja-arte" src={row.store_cover_url} alt="" />
        ) : (
          <div className="hoja-arte hoja-arte-vacia" aria-hidden="true" />
        )}
        <div className="hoja-cuerpo ficha">{ficha}</div>
      </div>
    </dialog>
  );
}
