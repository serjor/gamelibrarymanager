import { useState } from "react";
import { api, errorMessage, type LibraryRow, type PlayStatus } from "../../lib/api";

const STATUSES: { value: PlayStatus; label: string }[] = [
  { value: "backlog", label: "Pendiente" },
  { value: "playing", label: "Jugando" },
  { value: "finished", label: "Terminado" },
  { value: "abandoned", label: "Abandonado" },
];

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
 */
export function GameDetail({
  row,
  onClose,
  onSaved,
}: {
  row: LibraryRow;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [status, setStatus] = useState<PlayStatus | null>(row.status);
  const [rating, setRating] = useState<number | null>(row.rating);
  const [notes, setNotes] = useState(row.notes ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <aside className="detail">
      <header>
        <h2>{row.title}</h2>
        <button className="link" onClick={onClose} aria-label="Cerrar ficha">
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

      <label htmlFor="status">Estado</label>
      <select
        id="status"
        value={status ?? ""}
        onChange={(e) => setStatus((e.target.value || null) as PlayStatus | null)}
      >
        <option value="">Sin marcar</option>
        {STATUSES.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
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
    </aside>
  );
}
