import { useMemo, useState } from "react";
import type { LibraryRow } from "../../lib/api";
import { GameDetail } from "../game/GameDetail";
import { BulkBar } from "./BulkBar";
import { LibraryFilters } from "./LibraryFilters";
import { LibraryTable } from "./LibraryTable";
import { LibraryWall } from "./LibraryWall";
import { EMPTY_FILTERS, applyFilters, collectGenres, collectStores, type Filters } from "./filters";
import { DEFAULT_SORT, applySort } from "./sort";

/** Dos maneras de mirar lo mismo, no dos conjuntos de juegos. */
export type Vista = "tabla" | "pared";

export function Library({
  rows,
  vista,
  onVista,
  onSaved,
}: {
  rows: LibraryRow[];
  vista: Vista;
  onVista: (vista: Vista) => void;
  onSaved: () => void;
}) {
  const [filters, setFilters] = useState(EMPTY_FILTERS);
  const [sort, setSort] = useState(DEFAULT_SORT);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [abierto, setAbierto] = useState<string | null>(null);

  // Filtrado y orden se aplican una vez, aquí, y las dos vistas pintan el
  // resultado. Que cambiar de vista no pueda cambiar qué juegos hay delante no
  // es una comprobación que haya que hacer: es que no hay dos sitios donde
  // pudiera divergir.
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

  const compartido = {
    rows: visible,
    selected,
    onSelect: marcar,
    onOpen: (row: LibraryRow) => setAbierto(row.game_id),
    abierto,
  };

  return (
    <section className="library">
      <div className="barra">
        <LibraryFilters
          filters={filters}
          stores={stores}
          genres={genres}
          total={rows.length}
          shown={visible.length}
          onChange={filtrar}
        />
        <div className="vistas" role="group" aria-label="Modo de vista">
          <button
            className={vista === "tabla" ? "vista activa" : "vista"}
            aria-pressed={vista === "tabla"}
            onClick={() => onVista("tabla")}
          >
            Tabla
          </button>
          <button
            className={vista === "pared" ? "vista activa" : "vista"}
            aria-pressed={vista === "pared"}
            onClick={() => onVista("pared")}
          >
            Portadas
          </button>
        </div>
      </div>

      <div className="library-body">
        <div className="library-main">
          {vista === "tabla" ? (
            <LibraryTable {...compartido} sort={sort} onSort={setSort} />
          ) : (
            <LibraryWall {...compartido} />
          )}
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
