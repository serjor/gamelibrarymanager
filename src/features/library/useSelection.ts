import { useRef } from "react";
import type { LibraryRow } from "../../lib/api";

/**
 * Marks the games one at a time, or a range with the shift key.
 *
 * It lives out of the table and out of the wall because the two share the same
 * selection: if each one kept its own anchor, a change of view in the middle of
 * a range would give two different results for the same action.
 */
export function useSelection(
  rows: LibraryRow[],
  selected: Set<string>,
  onSelect: (gameIds: string[], checked: boolean) => void,
) {
  /** The point from which the range of the next ⇧+click counts. */
  const anchor = useRef<number | null>(null);

  return (index: number, withShift: boolean) => {
    const row = rows[index];
    if (!row) return;

    if (withShift && anchor.current !== null) {
      // The range counts over the games that you see, already filtered and
      // sorted, which is what the user has just pointed at with the mouse.
      const from = Math.min(anchor.current, index);
      const to = Math.max(anchor.current, index);
      onSelect(
        rows.slice(from, to + 1).map((r) => r.game_id),
        true,
      );
      return;
    }

    anchor.current = index;
    onSelect([row.game_id], !selected.has(row.game_id));
  };
}

/** The key goes in the native event; from the keyboard it comes without it. */
export function withShift(event: { nativeEvent: Event }): boolean {
  return (event.nativeEvent as MouseEvent).shiftKey === true;
}
