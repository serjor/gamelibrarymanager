import { useEffect, useMemo, useState } from "react";
import type { LibraryRow } from "../../lib/api";
import { GameDetail, type Presentation } from "../game/GameDetail";
import { BulkBar } from "./BulkBar";
import { LibraryFilters } from "./LibraryFilters";
import { LibraryTable } from "./LibraryTable";
import { LibraryWall } from "./LibraryWall";
import { EMPTY_FILTERS, applyFilters, collectGenres, collectStores, type Filters } from "./filters";
import { DEFAULT_SORT, applySort } from "./sort";

/** Two ways to look at the same games, not two sets of games. */
export type View = "table" | "wall";

/**
 * The width at which the inspector goes beside the table.
 *
 * It is not a round number and it does not come from the window of
 * `tauri.conf.json`: it is the sum of what each piece needs to operate. The
 * table stops being usable below 56rem (the table in styles.css), the inspector
 * is 22rem (the detail panel), the gap is 1rem, the rail is 14rem, and the
 * workspace uses 3rem of padding. The layout allowance includes the scroll bar
 * and totals 96rem, which counts in what a media query measures and not in what
 * stays for the window.
 *
 * Below that width, the game record opens as a sheet. The other option — to keep
 * the inspector and let the table scroll horizontally beside it — cuts the title
 * to "Ba…" exactly when you compare records, which is when you most need to read
 * it.
 */
const INSPECTOR_FITS = "(min-width: 96rem)";

/** Where ↑↓ already means something: the cursor of a text, the options of a list. */
function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.tagName === "TEXTAREA" || target.tagName === "SELECT") return true;
  return target.tagName === "INPUT" && (target as HTMLInputElement).type !== "checkbox";
}

export function Library({
  rows,
  view,
  onView,
  onSaved,
}: {
  rows: LibraryRow[];
  view: View;
  onView: (view: View) => void;
  onSaved: (rows: LibraryRow[]) => void;
}) {
  const [filters, setFilters] = useState(EMPTY_FILTERS);
  const [sort, setSort] = useState(DEFAULT_SORT);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [opened, setOpened] = useState<string | null>(null);
  const fits = useFits(INSPECTOR_FITS);

  // The filter and the sort are applied one time, here, and the two views show
  // the result. That a change of view cannot change which games are in front of
  // you is not a condition to test: there are not two places where they could
  // become different.
  const visible = useMemo(
    () => applySort(applyFilters(rows, filters), sort),
    [rows, filters, sort],
  );
  const stores = useMemo(() => collectStores(rows), [rows]);
  const genres = useMemo(() => collectGenres(rows), [rows]);
  const openedRow = useMemo(
    () => rows.find((row) => row.game_id === opened) ?? null,
    [rows, opened],
  );

  const mark = (gameIds: string[], checked: boolean) =>
    setSelected((previous) => {
      const next = new Set(previous);
      for (const id of gameIds) {
        if (checked) next.add(id);
        else next.delete(id);
      }
      return next;
    });

  // A change of the filter empties the selection. Without that, the selected
  // games stay there and you do not see them, and the bulk bar would write on
  // games that are no longer on the screen. That is the kind of surprise that
  // makes a user distrust a button that touches four hundred records.
  const filter = (next: Filters) => {
    setFilters(next);
    setSelected(new Set());
  };

  const shared = {
    rows: visible,
    selected,
    onSelect: mark,
    onOpen: (row: LibraryRow) => setOpened(row.game_id),
    opened,
  };

  // From the wall it is always a sheet: the inspector wastes the art, which is
  // the only thing that makes that view different. From the table, it is beside
  // the table while it fits.
  const presentation: Presentation = view === "wall" || !fits ? "sheet" : "inspector";

  // ↑↓ goes through the list and does not close the game record, which is the
  // reason that the inspector exists: to compare games one at a time without you
  // go back to the table to find the next one.
  //
  // The listener is on the window and not on the table because the table is
  // virtual: at a change of game, the row that had the focus can stop being
  // shown, the focus falls to the `body` and the second key press would find
  // nobody who listens.
  useEffect(() => {
    if (opened === null || presentation !== "inspector") return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      if (isTyping(event.target)) return;

      const current = visible.findIndex((row) => row.game_id === opened);
      const next = visible[current + (event.key === "ArrowDown" ? 1 : -1)];
      if (current === -1 || next === undefined) return;

      event.preventDefault();
      setOpened(next.game_id);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [opened, presentation, visible]);

  const record = openedRow && (
    <GameDetail
      key={openedRow.game_id}
      row={openedRow}
      variant={presentation}
      onClose={() => setOpened(null)}
      onSaved={onSaved}
    />
  );

  return (
    <section className="library command-deck">
      <div className="bar command-toolbar library-toolbar">
        <LibraryFilters
          filters={filters}
          stores={stores}
          genres={genres}
          total={rows.length}
          shown={visible.length}
          onChange={filter}
        />
        <div className="views" role="group" aria-label="View mode">
          <button
            className={view === "table" ? "view active" : "view"}
            aria-pressed={view === "table"}
            onClick={() => onView("table")}
          >
            Table
          </button>
          <button
            className={view === "wall" ? "view active" : "view"}
            aria-pressed={view === "wall"}
            onClick={() => onView("wall")}
          >
            Covers
          </button>
        </div>
      </div>

      <div className="library-body">
        <div className="library-main">
          {view === "table" ? (
            <LibraryTable {...shared} sort={sort} onSort={setSort} />
          ) : (
            <LibraryWall {...shared} />
          )}
          <BulkBar
            rows={visible}
            selected={selected}
            onSaved={onSaved}
            onClear={() => setSelected(new Set())}
          />
        </div>
        {presentation === "inspector" && record}
      </div>
      {/* The sheet covers all of the screen, thus it is attached to the section
          and not to the row where the inspector lives. */}
      {presentation === "sheet" && record}
    </section>
  );
}

/**
 * Whether the window is wide enough, measured against the browser and not
 * assumed.
 *
 * `matchMedia` and not a `ResizeObserver` on the container: what you must know is
 * whether the window is sufficient for the two pieces, and you know that before
 * you show either of them. With a measurement of the container you would have to
 * show the inspector to find that it did not fit.
 */
function useFits(query: string): boolean {
  const [fits, setFits] = useState(() => window.matchMedia(query).matches);

  useEffect(() => {
    const media = window.matchMedia(query);
    const onChange = () => setFits(media.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [query]);

  return fits;
}
