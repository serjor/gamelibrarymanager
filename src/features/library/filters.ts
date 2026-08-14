import type { LibraryRow, PlayStatus } from "../../lib/api";

export interface Filters {
  search: string;
  store: string | null;
  status: PlayStatus | "unset" | null;
  genre: string | null;
}

export const EMPTY_FILTERS: Filters = {
  search: "",
  store: null,
  status: null,
  genre: null,
};

/** Sin acentos ni mayúsculas: buscar "pokemon" tiene que encontrar "Pokémon". */
function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

/**
 * Filtrado puro, fuera de React: es lo que se ejecuta en cada tecla sobre miles
 * de filas, y así se puede probar sin montar nada.
 */
export function applyFilters(rows: LibraryRow[], filters: Filters): LibraryRow[] {
  const needle = fold(filters.search.trim());

  return rows.filter((row) => {
    if (needle && !fold(row.title).includes(needle)) return false;
    if (filters.store && !row.owned_stores.includes(filters.store)) return false;
    if (filters.genre && !row.genres.includes(filters.genre)) return false;
    if (filters.status === "unset" && row.status !== null) return false;
    if (filters.status && filters.status !== "unset" && row.status !== filters.status) return false;
    return true;
  });
}

export function collectStores(rows: LibraryRow[]): string[] {
  return [...new Set(rows.flatMap((row) => row.owned_stores))].sort();
}

export function collectGenres(rows: LibraryRow[]): string[] {
  return [...new Set(rows.flatMap((row) => row.genres))].sort();
}
