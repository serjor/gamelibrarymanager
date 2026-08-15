import { useMemo, useState } from "react";
import type { LibraryRow } from "../../lib/api";
import { STATUS_LABEL } from "../../lib/status";
import { GameDetail } from "../game/GameDetail";
import { WIDTH, COVER_HEIGHT } from "../library/LibraryWall";
import { featured, shelves } from "./shelves";

/**
 * What to play today.
 *
 * It is not a third view mode of the library, thus it does not read the library
 * filters: the table and the wall share a contract — you filter and the two show
 * the filtered games — while this screen makes its own divisions. Over shelves
 * that a rule selected, "genre = RPG" would mean nothing.
 *
 * The game record always opens as a sheet: there is no list beside it to keep in
 * view here, and what you look at is the art.
 */
export function Today({ rows, onSaved }: { rows: LibraryRow[]; onSaved: () => void }) {
  const [opened, setOpened] = useState<string | null>(null);
  // Held at the mount: the shelves divide at "six months ago", and a clock that
  // moves in the middle of a render would make two calculations of the same
  // screen disagree.
  const [now] = useState(() => Math.floor(Date.now() / 1000));

  const proposal = useMemo(() => featured(rows, now), [rows, now]);
  // The featured game does not appear again below: to see it two times on the
  // same screen makes you think that they are two games. If a shelf becomes
  // empty because of that, it is not shown, which is what already occurs with
  // any other empty shelf.
  const shelfList = useMemo(
    () => shelves(rows.filter((row) => row.game_id !== proposal?.game.game_id), now),
    [rows, proposal, now],
  );
  const openedRow = useMemo(
    () => rows.find((row) => row.game_id === opened) ?? null,
    [rows, opened],
  );

  if (proposal === null) {
    return (
      <p className="hint">
        There is not yet an owned game to propose. Synchronise a store and this
        screen will show what to play.
      </p>
    );
  }

  const game = proposal.game;

  return (
    <section className="today">
      <article className="featured">
        <div className="featured-art">
          {game.cover_url ? (
            <img src={game.cover_url} alt="" />
          ) : (
            // Decoration: the title is immediately beside it, and to repeat it
            // would make a screen reader say it two times.
            <span className="cover-placeholder" aria-hidden="true">
              {game.title}
            </span>
          )}
        </div>

        <div className="featured-text">
          <p className="hint">{proposal.reason}</p>
          <h2>{game.title}</h2>
          <p className="hint">
            {game.release_year ?? "year unknown"}
            {game.genres.length > 0 && ` · ${game.genres.join(", ")}`}
            {` · ${game.owned_stores.join(", ")}`}
            {game.status && ` · ${STATUS_LABEL[game.status]}`}
          </p>
          {game.summary && <p className="synopsis">{game.summary}</p>}
          <div className="actions">
            <button onClick={() => setOpened(game.game_id)}>Open the record</button>
          </div>
        </div>
      </article>

      {shelfList.map((shelf) => (
        <section key={shelf.id} className="shelf-box">
          <h3>{shelf.title}</h3>
          <p className="hint">{shelf.reason}</p>
          <ul
            className="shelf"
            style={{
              gridAutoColumns: `${WIDTH}px`,
              ["--cover-height" as string]: `${COVER_HEIGHT}px`,
            }}
          >
            {shelf.games.map((row) => (
              <li key={row.game_id}>
                <button className="tile" onClick={() => setOpened(row.game_id)}>
                  {row.cover_url ? (
                    <img src={row.cover_url} alt="" loading="lazy" />
                  ) : (
                    <span className="cover-placeholder" aria-hidden="true">
                      {row.title}
                    </span>
                  )}
                  <span className="tile-title">{row.title}</span>
                  <span className="hint">{row.owned_stores.join(" · ")}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}

      {openedRow && (
        <GameDetail
          key={openedRow.game_id}
          row={openedRow}
          variant="sheet"
          onClose={() => setOpened(null)}
          onSaved={onSaved}
        />
      )}
    </section>
  );
}
