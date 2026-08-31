import { useEffect } from "react";
import type { LibraryRow } from "../../lib/api";
import { STATUS_LABEL } from "../../lib/status";
import { useVirtualGrid } from "./useVirtualGrid";
import { withShift, useSelection } from "./useSelection";
import type { Sort, SortField } from "./sort";
import { isNoLongerInStore } from "./filters";

/** A fixed row height: the virtual list is arithmetic, not measurement. */
const ROW_HEIGHT = 38;

interface Column {
  field: SortField;
  label: string;
  width: string;
  numeric?: boolean;
}

const COLUMNS: Column[] = [
  { field: "title", label: "Title", width: "auto" },
  { field: "year", label: "Year", width: "4.5rem", numeric: true },
  { field: "genre", label: "Genre", width: "7rem" },
  { field: "stores", label: "Stores", width: "6.5rem" },
  { field: "hours", label: "Hours", width: "5rem", numeric: true },
  { field: "last", label: "Last", width: "5rem", numeric: true },
  { field: "status", label: "Status", width: "6.5rem" },
  { field: "rating", label: "Rating", width: "4rem", numeric: true },
];

function hours(minutes: number): string {
  if (minutes === 0) return "—";
  if (minutes < 60) return `${minutes} min`;
  return `${Math.round(minutes / 60)} h`;
}

/** The year is sufficient to see whether a game has waited a long time, and it
 *  uses four characters. */
function yearOf(epoch: number | null): string {
  return epoch === null ? "—" : String(new Date(epoch * 1000).getFullYear());
}

/**
 * The library as a table: dense, sortable and with multiple selection.
 *
 * It is virtual, with spacer rows above and below, and it does not move the
 * `<tbody>` with `transform` as the wall does. In a table that would take the
 * cells out of their column; a `<tr>` with a height uses the space and stays in
 * the table model, and the headers continue to align with the cells.
 *
 * It is a true table and not `div`s with a `role`: the native semantics is what
 * the keyboard and the screen readers already know how to read.
 */
export function LibraryTable({
  rows,
  totalRows = rows.length,
  sort,
  onSort,
  selected,
  onSelect,
  onOpen,
  opened,
}: {
  rows: LibraryRow[];
  totalRows?: number;
  sort: Sort;
  onSort: (sort: Sort) => void;
  selected: Set<string>;
  /** Marks or clears together: one at a click, a range with the shift key. */
  onSelect: (gameIds: string[], checked: boolean) => void;
  onOpen: (row: LibraryRow) => void;
  opened: string | null;
}) {
  const { containerRef, totalHeight, offsetY, range } = useVirtualGrid({
    itemCount: rows.length,
    rowHeight: ROW_HEIGHT,
  });
  const onMark = useSelection(rows, selected, onSelect);

  // With ↑↓ the open row goes out of the virtual window as soon as you go down
  // twenty games, and the inspector shows a game that is no longer on the
  // screen. To bring the row into view is what makes the keyboard really go
  // through the list. The arithmetic is the same as the virtual list: the row
  // multiplied by the row height, with no measurement.
  useEffect(() => {
    const box = containerRef.current;
    if (box === null || opened === null) return;

    const index = rows.findIndex((row) => row.game_id === opened);
    if (index === -1) return;

    // The header stays at the top and would cover the row that has just come in.
    const header = box.querySelector("thead")?.clientHeight ?? 0;
    const top = index * ROW_HEIGHT;
    const bottom = top + ROW_HEIGHT;

    if (top - header < box.scrollTop) box.scrollTop = top - header;
    else if (bottom > box.scrollTop + box.clientHeight) {
      box.scrollTop = bottom - box.clientHeight;
    }
  }, [opened, rows, containerRef]);

  if (rows.length === 0) {
    return (
      <div className="table-viewport" ref={containerRef}>
        <div className="empty-state" role="status">
          <strong className="empty-state-title">
            {totalRows === 0 ? "Your library is empty" : "No games match this filter"}
          </strong>
          <p className="hint">
            {totalRows === 0
              ? "Synchronise a store to bring your owned and wished-for games into this archive."
              : "No game agrees with your filter."}
          </p>
          {totalRows > 0 && (
            <p className="hint">Change or clear a filter to see more games.</p>
          )}
        </div>
      </div>
    );
  }

  const visible = rows.slice(range.start, range.end);
  const spacer = { top: offsetY, bottom: totalHeight - offsetY - visible.length * ROW_HEIGHT };

  // A click on the column that already sorts inverts the direction; a change of
  // column always starts with the ascending direction.
  const sortBy = (field: SortField) =>
    onSort({ field, desc: sort.field === field ? !sort.desc : false });

  return (
    <div className="table-viewport" ref={containerRef}>
      <table className="table command-table" aria-label="Game library">
        <colgroup>
          <col style={{ width: "2.2rem" }} />
          {COLUMNS.map((column) => (
            <col key={column.field} style={{ width: column.width }} />
          ))}
        </colgroup>
        <thead>
          <tr>
            <th />
            {COLUMNS.map((column) => (
              <th
                key={column.field}
                className={column.numeric ? "num" : undefined}
                aria-sort={
                  sort.field === column.field
                    ? sort.desc
                      ? "descending"
                      : "ascending"
                    : "none"
                }
              >
                <button className="th" onClick={() => sortBy(column.field)}>
                  {column.label}
                  {sort.field === column.field && (
                    <span aria-hidden="true">{sort.desc ? " ↓" : " ↑"}</span>
                  )}
                </button>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          <tr style={{ height: spacer.top }} />
          {visible.map((row, i) => {
            const index = range.start + i;
            const checked = selected.has(row.game_id);
            return (
              <tr
                key={row.game_id}
                className={`${checked ? "checked" : ""} ${opened === row.game_id ? "open" : ""}`.trim()}
              >
                <td>
                  <input
                    type="checkbox"
                    checked={checked}
                    aria-label={`Select ${row.title}`}
                    onChange={(e) => onMark(index, withShift(e))}
                  />
                </td>
                <td className="tt">
                  <button className="cell" onClick={() => onOpen(row)}>
                    {row.title}
                  </button>
                </td>
                <td className="num">{row.release_year ?? "—"}</td>
                <td>{row.genres[0] ?? "—"}</td>
                <td>
                  {row.owned_stores.length > 0 ? (
                    <span className="store-list">
                      {row.owned_stores.map((store) => (
                        <span key={store} className="store">
                          {store}
                        </span>
                      ))}
                    </span>
                  ) : isNoLongerInStore(row) ? (
                    <span className="status gone">Not in a store</span>
                  ) : (
                    <span className="hint">wished for</span>
                  )}
                </td>
                <td className="num">{hours(row.playtime_minutes)}</td>
                <td className="num">{yearOf(row.last_played_at)}</td>
                <td>
                  {row.status ? (
                    <span className={`status ${row.status}`}>{STATUS_LABEL[row.status]}</span>
                  ) : (
                    <span className="hint">—</span>
                  )}
                </td>
                <td className="num">{row.rating ?? "—"}</td>
              </tr>
            );
          })}
          <tr style={{ height: spacer.bottom }} />
        </tbody>
      </table>
    </div>
  );
}
