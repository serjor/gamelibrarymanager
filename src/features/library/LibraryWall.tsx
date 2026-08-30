import type { LibraryRow } from "../../lib/api";
import { GameArtwork } from "../game/GameArtwork";
import { STATUS_LABEL } from "../../lib/status";
import { useVirtualGrid } from "./useVirtualGrid";
import { withShift, useSelection } from "./useSelection";
import { isNoLongerInStore } from "./filters";

/**
 * The measurements of the tile, in one place and in pixels.
 *
 * The cover has an explicit height and the column has an explicit width, and
 * **not** an `aspect-ratio`: a grid item that stretches gives no height to its
 * row, the row divides the height that stays in the container, and the covers
 * cover each other. 150×200 is the 3:4 ratio of the covers.
 *
 * They come from here and not from the CSS because they are the same
 * measurements with which the wall calculates which rows apply. The shelves of
 * "Today" import them and do not repeat them: a tile with a different size on
 * each screen would not be the same tile.
 */
export const WIDTH = 150;
export const COVER_HEIGHT = 200;
/**
 * The cover plus the label. The 60 are measured, not estimated: two lines of
 * title (31), the line of stores or the gone marker (18) and the two spaces of
 * 4 between the three.
 */
const TILE_HEIGHT = COVER_HEIGHT + 60;
const GAP = 12;

/**
 * The library as a wall of covers.
 *
 * It shares state with the table: the same filters, the same order and the same
 * selection. A change of view does not change which games are in front of you,
 * only how you look at them.
 */
export function LibraryWall({
  rows,
  totalRows = rows.length,
  selected,
  onSelect,
  onOpen,
  opened,
}: {
  rows: LibraryRow[];
  totalRows?: number;
  selected: Set<string>;
  onSelect: (gameIds: string[], checked: boolean) => void;
  onOpen: (row: LibraryRow) => void;
  opened: string | null;
}) {
  const { containerRef, columns, totalHeight, offsetY, range } = useVirtualGrid({
    itemCount: rows.length,
    rowHeight: TILE_HEIGHT + GAP,
    columnWidth: WIDTH + GAP,
  });
  const onMark = useSelection(rows, selected, onSelect);

  if (rows.length === 0) {
    return (
      <div className="wall-viewport" ref={containerRef}>
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

  return (
    <div className="wall-viewport" ref={containerRef}>
      <div style={{ height: totalHeight, position: "relative" }}>
        <ul
          className="wall"
          // While there is a selection, the check boxes stay in view: if they
          // showed only when the pointer went over them, you would not see what
          // is selected.
          data-selecting={selected.size > 0 ? "" : undefined}
          style={{
            transform: `translateY(${offsetY}px)`,
            gridTemplateColumns: `repeat(${columns}, ${WIDTH}px)`,
            gridAutoRows: `${TILE_HEIGHT}px`,
            gap: `${GAP}px`,
            ["--cover-height" as string]: `${COVER_HEIGHT}px`,
          }}
        >
          {rows.slice(range.start, range.end).map((row, i) => {
            const index = range.start + i;
            const checked = selected.has(row.game_id);
            return (
              <li
                key={row.game_id}
                className={`${checked ? "checked" : ""} ${opened === row.game_id ? "open" : ""}`.trim()}
              >
                <input
                  type="checkbox"
                  className="wall-badge"
                  checked={checked}
                  aria-label={`Select ${row.title}`}
                  onChange={(e) => onMark(index, withShift(e))}
                />
                {row.status && (
                  <i className={`dot ${row.status}`} aria-hidden="true" />
                )}
                <button className="tile" onClick={() => onOpen(row)}>
                  <GameArtwork row={row} loading="lazy" />
                  <span className="tile-title">{row.title}</span>
                  {isNoLongerInStore(row) ? (
                    <span className="status gone">Not in a store</span>
                  ) : (
                    <span className="hint">
                      {row.owned_stores.join(" · ") || "only in the wishlist"}
                    </span>
                  )}
                  {/* A person who uses a screen reader does not see the colour
                      of the dot, thus the status also goes in the name. */}
                  {row.status && (
                    <span className="visually-hidden">
                      {STATUS_LABEL[row.status]}
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
}
