import type { LibraryRow } from "../../lib/api";
import { useVirtualGrid } from "./useVirtualGrid";

const ROW_HEIGHT = 250;
const COLUMN_WIDTH = 170;

export function LibraryGrid({
  rows,
  onSelect,
}: {
  rows: LibraryRow[];
  onSelect: (row: LibraryRow) => void;
}) {
  const { containerRef, columns, totalHeight, offsetY, range } = useVirtualGrid({
    itemCount: rows.length,
    rowHeight: ROW_HEIGHT,
    columnWidth: COLUMN_WIDTH,
  });

  if (rows.length === 0) {
    return <p className="hint">Ningún juego encaja con lo que has filtrado.</p>;
  }

  return (
    <div className="grid-viewport" ref={containerRef}>
      <div style={{ height: totalHeight, position: "relative" }}>
        <ul
          className="grid"
          style={{
            transform: `translateY(${offsetY}px)`,
            gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
          }}
        >
          {rows.slice(range.start, range.end).map((row) => (
            <li key={row.game_id}>
              <button onClick={() => onSelect(row)} className="card">
                {row.cover_url ? (
                  <img src={row.cover_url} alt="" loading="lazy" />
                ) : (
                  // Decorativo: el título ya está debajo, y repetirlo obliga a
                  // un lector de pantalla a leerlo dos veces por cada juego.
                  <span className="cover-placeholder" aria-hidden="true">
                    {row.title}
                  </span>
                )}
                <span className="card-title">{row.title}</span>
                <span className="hint">
                  {row.owned_stores.join(" · ") || "solo en deseados"}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
