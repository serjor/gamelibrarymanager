import { useRef } from "react";
import type { LibraryRow } from "../../lib/api";

/**
 * Marcar de uno en uno o por rango con mayúsculas.
 *
 * Vive fuera de la tabla y de la pared porque las dos comparten la misma
 * selección: si cada una llevara su cuenta del ancla, cambiar de vista a mitad
 * de un rango daría dos comportamientos distintos para el mismo gesto.
 */
export function useSeleccion(
  rows: LibraryRow[],
  selected: Set<string>,
  onSelect: (gameIds: string[], marcar: boolean) => void,
) {
  /** Desde dónde cuenta el rango del siguiente ⇧+clic. */
  const ancla = useRef<number | null>(null);

  return (indice: number, conMayusculas: boolean) => {
    const fila = rows[indice];
    if (!fila) return;

    if (conMayusculas && ancla.current !== null) {
      // El rango se cuenta sobre lo que se está viendo, ya filtrado y ordenado,
      // que es lo que el usuario acaba de señalar con el ratón.
      const desde = Math.min(ancla.current, indice);
      const hasta = Math.max(ancla.current, indice);
      onSelect(
        rows.slice(desde, hasta + 1).map((r) => r.game_id),
        true,
      );
      return;
    }

    ancla.current = indice;
    onSelect([fila.game_id], !selected.has(fila.game_id));
  };
}

/** La tecla viaja en el evento nativo; con el teclado llega sin ella. */
export function conMayusculas(evento: { nativeEvent: Event }): boolean {
  return (evento.nativeEvent as MouseEvent).shiftKey === true;
}
