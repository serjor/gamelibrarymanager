import { useMemo, useState } from "react";
import type { LibraryRow } from "../../lib/api";
import { GameDetail } from "../game/GameDetail";
import { LibraryFilters } from "./LibraryFilters";
import { LibraryGrid } from "./LibraryGrid";
import { EMPTY_FILTERS, applyFilters, collectGenres, collectStores } from "./filters";

export function Library({ rows, onSaved }: { rows: LibraryRow[]; onSaved: () => void }) {
  const [filters, setFilters] = useState(EMPTY_FILTERS);
  const [selected, setSelected] = useState<string | null>(null);

  const visible = useMemo(() => applyFilters(rows, filters), [rows, filters]);
  const stores = useMemo(() => collectStores(rows), [rows]);
  const genres = useMemo(() => collectGenres(rows), [rows]);
  const selectedRow = useMemo(
    () => rows.find((row) => row.game_id === selected) ?? null,
    [rows, selected],
  );

  return (
    <section className="library">
      <LibraryFilters
        filters={filters}
        stores={stores}
        genres={genres}
        total={rows.length}
        shown={visible.length}
        onChange={setFilters}
      />
      <div className="library-body">
        <LibraryGrid rows={visible} onSelect={(row) => setSelected(row.game_id)} />
        {selectedRow && (
          <GameDetail
            key={selectedRow.game_id}
            row={selectedRow}
            onClose={() => setSelected(null)}
            onSaved={onSaved}
          />
        )}
      </div>
    </section>
  );
}
