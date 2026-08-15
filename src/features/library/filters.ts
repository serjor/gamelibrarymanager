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

/** No accents and no capitals: a search for "pokemon" must find "Pokémon". */
function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

/**
 * A pure filter, out of React: it runs at each key press over thousands of rows,
 * and thus you can test it without you build a component.
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
