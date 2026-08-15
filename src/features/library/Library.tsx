import { useMemo, useState } from "react";
import type { LibraryRow } from "../../lib/api";
import { GameDetail } from "../game/GameDetail";
import { BulkBar } from "./BulkBar";
import { LibraryFilters } from "./LibraryFilters";
import { LibraryTable } from "./LibraryTable";
import { EMPTY_FILTERS, applyFilters, collectGenres, collectStores, type Filters } from "./filters";
import { DEFAULT_SORT, applySort } from "./sort";

export function Library({ rows, onSaved }: { rows: LibraryRow[]; onSaved: () => void }) {
  const [filters, setFilters] = useState(EMPTY_FILTERS);
  const [sort, setSort] = useState(DEFAULT_SORT);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [abierto, setAbierto] = useState<string | null>(null);

  const visible = useMemo(
    () => applySort(applyFilters(rows, filters), sort),
    [rows, filters, sort],
  );
  const stores = useMemo(() => collectStores(rows), [rows]);
  const genres = useMemo(() => collectGenres(rows), [rows]);
  const abiertoRow = useMemo(
    () => rows.find((row) => row.game_id === abierto) ?? null,
    [rows, abierto],
  );

  const marcar = (gameIds: string[], marcar: boolean) =>
    setSelected((previos) => {
      const siguiente = new Set(previos);
      for (const id of gameIds) {
        if (marcar) siguiente.add(id);
        else siguiente.delete(id);
      }
      return siguiente;
    });

  // Cambiar el filtro vacía la selección. Si no, lo seleccionado sigue ahí sin
  // verse y la barra de lote acabaría escribiendo sobre juegos que ya no están
  // en pantalla, que es la clase de sorpresa que hace desconfiar de un botón
  // que toca cuatrocientas fichas.
  const filtrar = (siguientes: Filters) => {
    setFilters(siguientes);
    setSelected(new Set());
  };

  return (
    <section className="library">
      <LibraryFilters
        filters={filters}
        stores={stores}
        genres={genres}
        total={rows.length}
        shown={visible.length}
        onChange={filtrar}
      />
      <div className="library-body">
        <div className="library-main">
          <LibraryTable
            rows={visible}
            sort={sort}
            onSort={setSort}
            selected={selected}
            onSelect={marcar}
            onOpen={(row) => setAbierto(row.game_id)}
            abierto={abierto}
          />
          <BulkBar
            rows={visible}
            selected={selected}
            onSaved={onSaved}
            onClear={() => setSelected(new Set())}
          />
        </div>
        {abiertoRow && (
          <GameDetail
            key={abiertoRow.game_id}
            row={abiertoRow}
            onClose={() => setAbierto(null)}
            onSaved={onSaved}
          />
        )}
      </div>
    </section>
  );
}
