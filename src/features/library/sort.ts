import type { LibraryRow } from "../../lib/api";
import { ESTADOS } from "../../lib/estado";

export type SortField =
  | "title"
  | "year"
  | "genre"
  | "stores"
  | "hours"
  | "last"
  | "status"
  | "rating";

export interface Sort {
  field: SortField;
  desc: boolean;
}

export const DEFAULT_SORT: Sort = { field: "title", desc: false };

/**
 * Lo que se compara de cada fila, o `null` cuando no hay dato.
 *
 * Cero horas y «nunca jugado» se tratan como falta de dato a propósito, igual
 * que hace la consulta con el `rtime_last_played: 0` de Steam: quien ordena por
 * horas quiere ver primero lo que ha jugado, no una tapia de ceros.
 */
function valor(row: LibraryRow, field: SortField): string | number | null {
  switch (field) {
    case "title":
      return row.sort_title;
    case "year":
      return row.release_year;
    case "genre":
      return row.genres[0] ?? null;
    case "stores":
      return row.owned_stores.join(" ") || null;
    case "hours":
      return row.playtime_minutes || null;
    case "last":
      return row.last_played_at;
    case "status":
      return row.status === null ? null : ESTADOS.indexOf(row.status);
    case "rating":
      return row.rating;
  }
}

/** El título decide siempre que dos filas empatan, para que el orden no baile. */
function desempate(a: LibraryRow, b: LibraryRow): number {
  return a.sort_title.localeCompare(b.sort_title, "es");
}

/**
 * Ordenación pura, fuera de React, como el filtrado.
 *
 * Lo que no tiene dato va al final **en los dos sentidos**. Invertir el orden
 * para ver los juegos con menos horas no debería llenar la primera pantalla de
 * los que no has abierto nunca: eso no es «pocas horas», es «no hay dato», y
 * son dos preguntas distintas.
 */
export function applySort(rows: LibraryRow[], sort: Sort): LibraryRow[] {
  const sentido = sort.desc ? -1 : 1;

  return [...rows].sort((a, b) => {
    const va = valor(a, sort.field);
    const vb = valor(b, sort.field);

    if (va === null && vb === null) return desempate(a, b);
    if (va === null) return 1;
    if (vb === null) return -1;

    const comparados =
      typeof va === "string" && typeof vb === "string"
        ? va.localeCompare(vb, "es")
        : Number(va) - Number(vb);

    return comparados === 0 ? desempate(a, b) : comparados * sentido;
  });
}
