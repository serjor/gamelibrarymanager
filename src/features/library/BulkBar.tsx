import { useState } from "react";
import { api, errorMessage, type LibraryRow, type PlayStatus } from "../../lib/api";
import { ESTADOS, ETIQUETA_ESTADO } from "../../lib/estado";

/**
 * Lo que se puede hacerle de golpe a lo seleccionado.
 *
 * Es la razón de que la biblioteca sea una tabla: con cuatrocientas fichas,
 * marcar treinta como abandonadas de una en una no lo hace nadie, y por eso el
 * estado se queda sin poner.
 */
export function BulkBar({
  rows,
  selected,
  onSaved,
  onClear,
}: {
  /** Todas las filas, para recuperar de cada una lo que no se está cambiando. */
  rows: LibraryRow[];
  selected: Set<string>;
  onSaved: () => void;
  onClear: () => void;
}) {
  const [estado, setEstado] = useState<PlayStatus | "">("");
  const [ocupado, setOcupado] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const cuantos = selected.size;
  if (cuantos === 0) return null;

  const aplicar = async () => {
    setOcupado(true);
    setError(null);
    try {
      for (const row of rows.filter((r) => selected.has(r.game_id))) {
        // `set_user_state` reescribe la fila entera, así que la nota y el texto
        // hay que devolvérselos tal cual: sin esto, poner un estado en lote
        // borraría en silencio todo lo que el usuario tenga escrito, que es
        // justo lo único que esta aplicación sabe de él y no sabe la tienda.
        await api.setUserState(row.game_id, estado || null, row.rating, row.notes);
      }
      onClear();
      onSaved();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setOcupado(false);
    }
  };

  return (
    <div className="lote">
      <strong>
        {cuantos} seleccionado{cuantos === 1 ? "" : "s"}
      </strong>

      <label className="lote-campo" htmlFor="lote-estado">
        Marcar como
      </label>
      <select
        id="lote-estado"
        value={estado}
        onChange={(e) => setEstado(e.target.value as PlayStatus | "")}
      >
        <option value="">Sin marcar</option>
        {ESTADOS.map((valor) => (
          <option key={valor} value={valor}>
            {ETIQUETA_ESTADO[valor]}
          </option>
        ))}
      </select>

      <button disabled={ocupado} onClick={() => void aplicar()}>
        {ocupado ? "Aplicando…" : "Aplicar"}
      </button>
      <button className="link" onClick={onClear}>
        deseleccionar
      </button>

      {error && <p role="alert">{error}</p>}
    </div>
  );
}
