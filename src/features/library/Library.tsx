import { useEffect, useMemo, useState } from "react";
import type { LibraryRow } from "../../lib/api";
import { GameDetail, type Presentacion } from "../game/GameDetail";
import { BulkBar } from "./BulkBar";
import { LibraryFilters } from "./LibraryFilters";
import { LibraryTable } from "./LibraryTable";
import { LibraryWall } from "./LibraryWall";
import { EMPTY_FILTERS, applyFilters, collectGenres, collectStores, type Filters } from "./filters";
import { DEFAULT_SORT, applySort } from "./sort";

/** Dos maneras de mirar lo mismo, no dos conjuntos de juegos. */
export type Vista = "tabla" | "pared";

/**
 * El ancho a partir del cual el inspector cabe al lado de la tabla.
 *
 * No es un número redondo ni sale de la ventana de `tauri.conf.json`: es la
 * suma de lo que cada pieza necesita para funcionar. La tabla deja de servir
 * por debajo de 56rem (`.tabla` en `styles.css`), el inspector mide 20rem
 * (`.detail`), entre los dos hay 1rem de hueco (`--e-5`) y `main` se lleva 3rem
 * de relleno: 80rem. Los 2rem de más son para la barra de desplazamiento, que
 * entra en lo que mide una media query y no en lo que le queda a `main`.
 *
 * Por debajo, la ficha se abre como hoja. La alternativa —dejar el inspector y
 * que la tabla se desplace en horizontal a su lado— recorta el título a «Ba…»
 * justo cuando estás comparando fichas, que es cuando más falta hace leerlo.
 */
const CABE_EL_INSPECTOR = "(min-width: 82rem)";

/** Donde ↑↓ ya significa algo: el cursor de un texto, las opciones de una lista. */
function escribiendo(destino: EventTarget | null): boolean {
  if (!(destino instanceof HTMLElement)) return false;
  if (destino.tagName === "TEXTAREA" || destino.tagName === "SELECT") return true;
  return destino.tagName === "INPUT" && (destino as HTMLInputElement).type !== "checkbox";
}

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
  const cabe = useCabe(CABE_EL_INSPECTOR);

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

  // Desde la pared siempre en hoja: el inspector desaprovecha el arte, que es
  // lo único que distingue esa vista. Desde la tabla, acoplado mientras quepa.
  const presentacion: Presentacion = vista === "pared" || !cabe ? "sheet" : "inspector";

  // ↑↓ recorre la lista sin cerrar la ficha, que es la razón de que el
  // inspector exista: comparar juegos de uno en uno sin volver a la tabla a
  // buscar el siguiente.
  //
  // El oyente va en la ventana y no en la tabla porque la tabla está
  // virtualizada: al cambiar de juego, la fila que tenía el foco puede dejar de
  // estar pintada, el foco se cae al `body` y la segunda pulsación ya no
  // encontraría a nadie escuchando.
  useEffect(() => {
    if (abierto === null || presentacion !== "inspector") return;

    const alPulsar = (evento: KeyboardEvent) => {
      if (evento.key !== "ArrowDown" && evento.key !== "ArrowUp") return;
      if (escribiendo(evento.target)) return;

      const actual = visible.findIndex((row) => row.game_id === abierto);
      const siguiente = visible[actual + (evento.key === "ArrowDown" ? 1 : -1)];
      if (actual === -1 || siguiente === undefined) return;

      evento.preventDefault();
      setAbierto(siguiente.game_id);
    };

    window.addEventListener("keydown", alPulsar);
    return () => window.removeEventListener("keydown", alPulsar);
  }, [abierto, presentacion, visible]);

  const ficha = abiertoRow && (
    <GameDetail
      key={abiertoRow.game_id}
      row={abiertoRow}
      variant={presentacion}
      onClose={() => setAbierto(null)}
      onSaved={onSaved}
    />
  );

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
        {presentacion === "inspector" && ficha}
      </div>
      {/* La hoja se superpone a la pantalla entera, así que cuelga de la
          sección y no de la fila donde vive el inspector. */}
      {presentacion === "sheet" && ficha}
    </section>
  );
}

/**
 * Si la ventana da de sí, medido contra el navegador y no supuesto.
 *
 * `matchMedia` y no un `ResizeObserver` sobre el contenedor: lo que hay que
 * saber es si la ventana da para las dos piezas, y eso se sabe antes de pintar
 * ninguna. Midiendo el contenedor habría que pintar el inspector para
 * descubrir que no cabía.
 */
function useCabe(consulta: string): boolean {
  const [cabe, setCabe] = useState(() => window.matchMedia(consulta).matches);

  useEffect(() => {
    const media = window.matchMedia(consulta);
    const alCambiar = () => setCabe(media.matches);
    media.addEventListener("change", alCambiar);
    return () => media.removeEventListener("change", alCambiar);
  }, [consulta]);

  return cabe;
}
