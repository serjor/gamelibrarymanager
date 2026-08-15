import type { LibraryRow } from "../../lib/api";
import { STATUSES } from "../../lib/status";

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
 * What the sort compares in each row, or `null` when there is no data.
 *
 * Zero hours and "never played" count as absent data, and that is deliberate. It
 * is the same as what the query does with the `rtime_last_played: 0` of Steam: a
 * user who sorts by hours wants to see first the games that they have played,
 * not a wall of zeros.
 */
function value(row: LibraryRow, field: SortField): string | number | null {
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
      return row.status === null ? null : STATUSES.indexOf(row.status);
    case "rating":
      return row.rating;
  }
}

/** The title always decides when two rows are equal, so that the order stays. */
function tieBreak(a: LibraryRow, b: LibraryRow): number {
  return a.sort_title.localeCompare(b.sort_title, "en");
}

/**
 * A pure sort, out of React, as the filter is.
 *
 * The rows with no data go last **in the two directions**. To invert the order
 * to see the games with the fewest hours must not fill the first screen with the
 * games that you have never opened: that is not "few hours", it is "no data",
 * and they are two different questions.
 */
export function applySort(rows: LibraryRow[], sort: Sort): LibraryRow[] {
  const direction = sort.desc ? -1 : 1;

  return [...rows].sort((a, b) => {
    const va = value(a, sort.field);
    const vb = value(b, sort.field);

    if (va === null && vb === null) return tieBreak(a, b);
    if (va === null) return 1;
    if (vb === null) return -1;

    const compared =
      typeof va === "string" && typeof vb === "string"
        ? va.localeCompare(vb, "en")
        : Number(va) - Number(vb);

    return compared === 0 ? tieBreak(a, b) : compared * direction;
  });
}
