import { useEffect } from "react";
import type { LibraryRow } from "../../lib/api";
import { ETIQUETA_ESTADO } from "../../lib/estado";
import { useVirtualGrid } from "./useVirtualGrid";
import { conMayusculas, useSeleccion } from "./useSeleccion";
import type { Sort, SortField } from "./sort";

/** Alto de fila fijo: la virtualización es aritmética, no medición. */
const ALTO_FILA = 33;

interface Columna {
  campo: SortField;
  etiqueta: string;
  ancho: string;
  numerica?: boolean;
}

const COLUMNAS: Columna[] = [
  { campo: "title", etiqueta: "Título", ancho: "auto" },
  { campo: "year", etiqueta: "Año", ancho: "4.5rem", numerica: true },
  { campo: "genre", etiqueta: "Género", ancho: "7rem" },
  { campo: "stores", etiqueta: "Tiendas", ancho: "6.5rem" },
  { campo: "hours", etiqueta: "Horas", ancho: "5rem", numerica: true },
  { campo: "last", etiqueta: "Últ.", ancho: "5rem", numerica: true },
  { campo: "status", etiqueta: "Estado", ancho: "6.5rem" },
  { campo: "rating", etiqueta: "Nota", ancho: "4rem", numerica: true },
];

function horas(minutos: number): string {
  if (minutos === 0) return "—";
  if (minutos < 60) return `${minutos} min`;
  return `${Math.round(minutos / 60)} h`;
}

/** El año basta para decidir si algo lleva mucho aparcado, y ocupa cinco letras. */
function anyoDe(epoch: number | null): string {
  return epoch === null ? "—" : String(new Date(epoch * 1000).getFullYear());
}

/**
 * La biblioteca como tabla: densa, ordenable y con selección múltiple.
 *
 * Va virtualizada con filas espaciadoras arriba y abajo, y no moviendo el
 * `<tbody>` con `transform` como hace la pared. Dentro de una tabla eso saca
 * las celdas de su columna; un `<tr>` con altura ocupa el sitio sin salirse del
 * modelo de tabla, y las cabeceras siguen cuadrando con las celdas.
 *
 * Es una tabla de verdad y no `div`s con `role`: la semántica nativa es la que
 * ya saben leer el teclado y los lectores de pantalla.
 */
export function LibraryTable({
  rows,
  sort,
  onSort,
  selected,
  onSelect,
  onOpen,
  abierto,
}: {
  rows: LibraryRow[];
  sort: Sort;
  onSort: (sort: Sort) => void;
  selected: Set<string>;
  /** Marca o desmarca de golpe: uno al pulsar, un rango con mayúsculas. */
  onSelect: (gameIds: string[], marcar: boolean) => void;
  onOpen: (row: LibraryRow) => void;
  abierto: string | null;
}) {
  const { containerRef, totalHeight, offsetY, range } = useVirtualGrid({
    itemCount: rows.length,
    rowHeight: ALTO_FILA,
  });
  const alMarcar = useSeleccion(rows, selected, onSelect);

  // Con ↑↓ la fila abierta se va de la ventana virtualizada en cuanto bajas
  // veinte juegos, y el inspector acaba enseñando uno que ya no está en
  // pantalla. Traerla a la vista es lo que hace que recorrer la lista con el
  // teclado sea recorrerla de verdad. La aritmética es la misma que virtualiza:
  // fila por alto de fila, sin medir nada.
  useEffect(() => {
    const caja = containerRef.current;
    if (caja === null || abierto === null) return;

    const indice = rows.findIndex((row) => row.game_id === abierto);
    if (indice === -1) return;

    // La cabecera va pegada arriba y taparía la fila que acaba de entrar.
    const cabecera = caja.querySelector("thead")?.clientHeight ?? 0;
    const arriba = indice * ALTO_FILA;
    const abajo = arriba + ALTO_FILA;

    if (arriba - cabecera < caja.scrollTop) caja.scrollTop = arriba - cabecera;
    else if (abajo > caja.scrollTop + caja.clientHeight) {
      caja.scrollTop = abajo - caja.clientHeight;
    }
  }, [abierto, rows, containerRef]);

  if (rows.length === 0) {
    return <p className="hint">Ningún juego encaja con lo que has filtrado.</p>;
  }

  const visibles = rows.slice(range.start, range.end);
  const alto = { arriba: offsetY, abajo: totalHeight - offsetY - visibles.length * ALTO_FILA };

  // Pulsar la columna por la que ya se ordena da la vuelta; cambiar de columna
  // empieza siempre ascendente.
  const ordenarPor = (campo: SortField) =>
    onSort({ field: campo, desc: sort.field === campo ? !sort.desc : false });

  return (
    <div className="tabla-viewport" ref={containerRef}>
      <table className="tabla">
        <colgroup>
          <col style={{ width: "2.2rem" }} />
          {COLUMNAS.map((columna) => (
            <col key={columna.campo} style={{ width: columna.ancho }} />
          ))}
        </colgroup>
        <thead>
          <tr>
            <th />
            {COLUMNAS.map((columna) => (
              <th
                key={columna.campo}
                className={columna.numerica ? "num" : undefined}
                aria-sort={
                  sort.field === columna.campo
                    ? sort.desc
                      ? "descending"
                      : "ascending"
                    : "none"
                }
              >
                <button className="th" onClick={() => ordenarPor(columna.campo)}>
                  {columna.etiqueta}
                  {sort.field === columna.campo && (
                    <span aria-hidden="true">{sort.desc ? " ↓" : " ↑"}</span>
                  )}
                </button>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          <tr style={{ height: alto.arriba }} />
          {visibles.map((row, i) => {
            const indice = range.start + i;
            const marcada = selected.has(row.game_id);
            return (
              <tr
                key={row.game_id}
                className={`${marcada ? "marcada" : ""} ${abierto === row.game_id ? "abierta" : ""}`.trim()}
              >
                <td>
                  <input
                    type="checkbox"
                    checked={marcada}
                    aria-label={`Seleccionar ${row.title}`}
                    onChange={(e) => alMarcar(indice, conMayusculas(e))}
                  />
                </td>
                <td className="tt">
                  <button className="celda" onClick={() => onOpen(row)}>
                    {row.title}
                  </button>
                </td>
                <td className="num">{row.release_year ?? "—"}</td>
                <td>{row.genres[0] ?? "—"}</td>
                <td>
                  {row.owned_stores.length > 0 ? (
                    row.owned_stores.map((tienda) => (
                      <span key={tienda} className="tienda">
                        {tienda}
                      </span>
                    ))
                  ) : (
                    <span className="hint">deseado</span>
                  )}
                </td>
                <td className="num">{horas(row.playtime_minutes)}</td>
                <td className="num">{anyoDe(row.last_played_at)}</td>
                <td>
                  {row.status ? (
                    <span className={`estado ${row.status}`}>{ETIQUETA_ESTADO[row.status]}</span>
                  ) : (
                    <span className="hint">—</span>
                  )}
                </td>
                <td className="num">{row.rating ?? "—"}</td>
              </tr>
            );
          })}
          <tr style={{ height: alto.abajo }} />
        </tbody>
      </table>
    </div>
  );
}
