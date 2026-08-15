import { useEffect, useRef, useState } from "react";
import { api, errorMessage, type LibraryRow, type PlayStatus } from "../../lib/api";
import { STATUSES, STATUS_LABEL } from "../../lib/status";

/**
 * Beside the table, or on top of the covers.
 *
 * They are not two records: it is the same record with two ways to show it. What
 * changes is the container and the art; the form, the save and the validation
 * are the same objects in the same place.
 */
export type Presentation = "inspector" | "sheet";

function hours(minutes: number): string {
  if (minutes === 0) return "not played";
  if (minutes < 60) return `${minutes} min`;
  return `${Math.round(minutes / 60)} h`;
}

/**
 * The unified record: the metadata, the stores that have the copy, and the only
 * data that the user writes. The save is explicit so that a note that is half
 * written does not go away when the panel closes.
 *
 * The state of the form is initialised one time and does not synchronise with
 * the props: the caller gives `key={row.game_id}`, thus a change of game builds
 * the panel again and no effect must copy props to state.
 *
 * There is only one call to `api.setUserState` in all of the file, and that is
 * the proof that the two presentations save through the same path: there is no
 * second place where one of them could start to validate differently from the
 * other.
 */
export function GameDetail({
  row,
  variant,
  onClose,
  onSaved,
}: {
  row: LibraryRow;
  variant: Presentation;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [status, setStatus] = useState<PlayStatus | null>(row.status);
  const [rating, setRating] = useState<number | null>(row.rating);
  const [notes, setNotes] = useState(row.notes ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sheet = useRef<HTMLDialogElement | null>(null);

  // A true modal and not a `div` with `role="dialog"`: the browser already keeps
  // the focus inside, closes with Escape and gives the focus back to where it
  // was, and made by hand that is one hundred lines that are incorrect in the
  // rare conditions.
  useEffect(() => {
    sheet.current?.showModal();
  }, []);

  // Close through the path of the browser when there is a dialog, which is what
  // gives the focus back to the tile from which it opened.
  const close = () => (sheet.current === null ? onClose() : sheet.current.close());

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.setUserState(row.game_id, status, rating, notes.trim() || null);
      onSaved();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  const card = (
    <>
      <header>
        <h2 id="card-title">{row.title}</h2>
        <button className="link" onClick={close} aria-label="Close record">
          close
        </button>
      </header>

      <p className="hint">
        {row.release_year ?? "year unknown"} · {hours(row.playtime_minutes)}
        {row.genres.length > 0 && ` · ${row.genres.join(", ")}`}
      </p>

      <p className="hint">
        {row.owned_stores.length > 0
          ? `Owned in: ${row.owned_stores.join(", ")}`
          : "You do not have it in a store"}
        {row.wishlist_stores.length > 0 && ` · Wished for in: ${row.wishlist_stores.join(", ")}`}
      </p>

      {/* The summary comes from IGDB, thus it is absent exactly in the records
          that came from the title of the store. To say that is better than an
          empty space with no words: it is the same promise as the message in
          the header. */}
      {row.summary ? (
        <p className="synopsis">{row.summary}</p>
      ) : (
        <p className="hint">No summary: the record was made with the title of the store.</p>
      )}

      <label htmlFor="status">Status</label>
      <select
        id="status"
        value={status ?? ""}
        onChange={(e) => setStatus((e.target.value || null) as PlayStatus | null)}
      >
        <option value="">Not marked</option>
        {STATUSES.map((value) => (
          <option key={value} value={value}>
            {STATUS_LABEL[value]}
          </option>
        ))}
      </select>

      <label htmlFor="rating">Rating (1-10)</label>
      <input
        id="rating"
        type="number"
        min={1}
        max={10}
        value={rating ?? ""}
        onChange={(e) => setRating(e.target.value === "" ? null : Number(e.target.value))}
      />

      <label htmlFor="notes">Notes</label>
      <textarea id="notes" rows={4} value={notes} onChange={(e) => setNotes(e.target.value)} />

      {error && <p role="alert">{error}</p>}

      <button onClick={() => void save()} disabled={busy}>
        {busy ? "Saving…" : "Save"}
      </button>
    </>
  );

  if (variant === "inspector") {
    return <aside className="detail card">{card}</aside>;
  }

  return (
    <dialog
      className="sheet"
      ref={sheet}
      aria-labelledby="card-title"
      onClose={onClose}
      // The dialog itself shows the scrim, thus a click out of the box comes
      // here with the dialog as the target and no coordinates must be measured.
      onClick={(event) => {
        if (event.target === sheet.current) close();
      }}
    >
      <div className="sheet-box">
        {/* Wide and from the store, which is what the sheet has and the
            inspector does not: the Steam header or the GOG logo, cut to the same
            box so that the record always starts at the same height.
            Decoration: the title is immediately below. */}
        {row.store_cover_url ? (
          <img className="sheet-art" src={row.store_cover_url} alt="" />
        ) : (
          <div className="sheet-art sheet-art-empty" aria-hidden="true" />
        )}
        <div className="sheet-body card">{card}</div>
      </div>
    </dialog>
  );
}
