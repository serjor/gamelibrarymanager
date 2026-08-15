import type { LibraryRow } from "../../lib/api";
import { ETIQUETA_ESTADO } from "../../lib/estado";
import { useVirtualGrid } from "./useVirtualGrid";
import { conMayusculas, useSeleccion } from "./useSeleccion";

/**
 * Medidas de la baldosa, en un solo sitio y en píxeles.
 *
 * La portada va con alto explícito y la columna con ancho explícito, **no** con
 * `aspect-ratio`: un item de rejilla estirado no aporta altura a su fila, la
 * fila se reparte el alto sobrante del contenedor y las portadas acaban
 * pisándose unas a otras. 126×168 es la proporción 3:4 de las carátulas.
 */
const ANCHO = 126;
const ALTO_PORTADA = 168;
/**
 * Portada más el rótulo. Los 56 son medidos, no estimados: dos líneas de
 * título (31), la línea de tiendas (14) y los dos huecos de 4 entre los tres.
 */
const ALTO_BALDOSA = ALTO_PORTADA + 56;
const HUECO = 12;

/**
 * La biblioteca como pared de portadas.
 *
 * Comparte estado con la tabla: los mismos filtros, el mismo orden y la misma
 * selección. Cambiar de vista no cambia qué juegos hay delante, solo cómo se
 * miran.
 */
export function LibraryWall({
  rows,
  selected,
  onSelect,
  onOpen,
  abierto,
}: {
  rows: LibraryRow[];
  selected: Set<string>;
  onSelect: (gameIds: string[], marcar: boolean) => void;
  onOpen: (row: LibraryRow) => void;
  abierto: string | null;
}) {
  const { containerRef, columns, totalHeight, offsetY, range } = useVirtualGrid({
    itemCount: rows.length,
    rowHeight: ALTO_BALDOSA + HUECO,
    columnWidth: ANCHO + HUECO,
  });
  const alMarcar = useSeleccion(rows, selected, onSelect);

  if (rows.length === 0) {
    return <p className="hint">Ningún juego encaja con lo que has filtrado.</p>;
  }

  return (
    <div className="pared-viewport" ref={containerRef}>
      <div style={{ height: totalHeight, position: "relative" }}>
        <ul
          className="pared"
          // Mientras haya algo marcado las casillas se quedan a la vista: si
          // solo salieran al pasar por encima, no se vería lo seleccionado.
          data-seleccionando={selected.size > 0 ? "" : undefined}
          style={{
            transform: `translateY(${offsetY}px)`,
            gridTemplateColumns: `repeat(${columns}, ${ANCHO}px)`,
            gridAutoRows: `${ALTO_BALDOSA}px`,
            gap: `${HUECO}px`,
            ["--alto-portada" as string]: `${ALTO_PORTADA}px`,
          }}
        >
          {rows.slice(range.start, range.end).map((row, i) => {
            const indice = range.start + i;
            const marcada = selected.has(row.game_id);
            return (
              <li
                key={row.game_id}
                className={`${marcada ? "marcada" : ""} ${abierto === row.game_id ? "abierta" : ""}`.trim()}
              >
                <input
                  type="checkbox"
                  className="pared-marca"
                  checked={marcada}
                  aria-label={`Seleccionar ${row.title}`}
                  onChange={(e) => alMarcar(indice, conMayusculas(e))}
                />
                {row.status && (
                  <i className={`punto ${row.status}`} aria-hidden="true" />
                )}
                <button className="baldosa" onClick={() => onOpen(row)}>
                  {row.cover_url ? (
                    <img src={row.cover_url} alt="" loading="lazy" />
                  ) : (
                    // Decorativo: el título ya está debajo, y repetirlo obliga a
                    // un lector de pantalla a leerlo dos veces por cada juego.
                    <span className="cover-placeholder" aria-hidden="true">
                      {row.title}
                    </span>
                  )}
                  <span className="baldosa-titulo">{row.title}</span>
                  <span className="hint">
                    {row.owned_stores.join(" · ") || "solo en deseados"}
                  </span>
                  {/* El color del punto no lo ve quien usa un lector de
                      pantalla, así que el estado va también en el nombre. */}
                  {row.status && (
                    <span className="visualmente-oculta">
                      {ETIQUETA_ESTADO[row.status]}
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
}
