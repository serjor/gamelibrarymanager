import type { PlayStatus } from "../../lib/api";
import type { Filters } from "./filters";

const STATUSES: { value: PlayStatus | "unset"; label: string }[] = [
  { value: "backlog", label: "Pendiente" },
  { value: "playing", label: "Jugando" },
  { value: "finished", label: "Terminado" },
  { value: "abandoned", label: "Abandonado" },
  { value: "unset", label: "Sin marcar" },
];

export function LibraryFilters({
  filters,
  stores,
  genres,
  total,
  shown,
  onChange,
}: {
  filters: Filters;
  stores: string[];
  genres: string[];
  total: number;
  shown: number;
  onChange: (filters: Filters) => void;
}) {
  return (
    <div className="filters">
      <input
        type="search"
        value={filters.search}
        onChange={(e) => onChange({ ...filters, search: e.target.value })}
        placeholder="Buscar en la biblioteca"
        aria-label="Buscar en la biblioteca"
      />

      <select
        value={filters.store ?? ""}
        onChange={(e) => onChange({ ...filters, store: e.target.value || null })}
        aria-label="Tienda"
      >
        <option value="">Todas las tiendas</option>
        {stores.map((store) => (
          <option key={store} value={store}>
            {store}
          </option>
        ))}
      </select>

      <select
        value={filters.status ?? ""}
        onChange={(e) =>
          onChange({ ...filters, status: (e.target.value || null) as Filters["status"] })
        }
        aria-label="Estado"
      >
        <option value="">Cualquier estado</option>
        {STATUSES.map((status) => (
          <option key={status.value} value={status.value}>
            {status.label}
          </option>
        ))}
      </select>

      <select
        value={filters.genre ?? ""}
        onChange={(e) => onChange({ ...filters, genre: e.target.value || null })}
        aria-label="Género"
      >
        <option value="">Cualquier género</option>
        {genres.map((genre) => (
          <option key={genre} value={genre}>
            {genre}
          </option>
        ))}
      </select>

      <span className="hint">
        {shown === total ? `${total} juegos` : `${shown} de ${total}`}
      </span>
    </div>
  );
}
