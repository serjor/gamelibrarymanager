import type { PlayStatus } from "../../lib/api";
import type { Filters } from "./filters";

const STATUSES: { value: PlayStatus | "unset"; label: string }[] = [
  { value: "backlog", label: "Backlog" },
  { value: "playing", label: "Playing" },
  { value: "finished", label: "Finished" },
  { value: "abandoned", label: "Abandoned" },
  { value: "unset", label: "Not marked" },
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
        placeholder="Search in the library"
        aria-label="Search in the library"
      />

      <select
        value={filters.store ?? ""}
        onChange={(e) => onChange({ ...filters, store: e.target.value || null })}
        aria-label="Store"
      >
        <option value="">All of the stores</option>
        {stores.map((store) => (
          <option key={store} value={store}>
            {store}
          </option>
        ))}
      </select>

      <select
        value={filters.availability ?? ""}
        onChange={(e) =>
          onChange({
            ...filters,
            availability: (e.target.value || null) as Filters["availability"],
          })
        }
        aria-label="Availability"
      >
        <option value="">All games</option>
        <option value="gone">No longer in a store</option>
      </select>

      <select
        value={filters.status ?? ""}
        onChange={(e) =>
          onChange({ ...filters, status: (e.target.value || null) as Filters["status"] })
        }
        aria-label="Status"
      >
        <option value="">Any status</option>
        {STATUSES.map((status) => (
          <option key={status.value} value={status.value}>
            {status.label}
          </option>
        ))}
      </select>

      <select
        value={filters.genre ?? ""}
        onChange={(e) => onChange({ ...filters, genre: e.target.value || null })}
        aria-label="Genre"
      >
        <option value="">Any genre</option>
        {genres.map((genre) => (
          <option key={genre} value={genre}>
            {genre}
          </option>
        ))}
      </select>

      <span className="hint">
        {shown === total ? `${total} games` : `${shown} of ${total}`}
      </span>
    </div>
  );
}
