import { useState } from "react";
import { api, errorMessage, type LibraryRow, type PlayStatus } from "../../lib/api";
import { STATUSES, STATUS_LABEL } from "../../lib/status";

/**
 * What you can do to the selection together.
 *
 * It is the reason that the library is a table: with four hundred records,
 * nobody marks thirty games as abandoned one at a time, and thus the status
 * stays empty.
 */
export function BulkBar({
  rows,
  selected,
  onSaved,
  onClear,
}: {
  /** All of the rows, to get from each one the data that does not change. */
  rows: LibraryRow[];
  selected: Set<string>;
  /** The rows that the save gave back, one call for all of the selection. */
  onSaved: (rows: LibraryRow[]) => void;
  onClear: () => void;
}) {
  const [status, setStatus] = useState<PlayStatus | "">("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const count = selected.size;
  if (count === 0) return null;

  const apply = async () => {
    setBusy(true);
    setError(null);
    try {
      // One call and one transaction. Thirty games were thirty commands, one
      // after another, and a failure in the middle left half of the selection
      // written and the other half not.
      //
      // `set_user_state` writes all of the row again, thus you must give the
      // rating and the notes back unchanged: without this, a bulk status would
      // quietly delete all of the text that the user wrote, which is exactly
      // the only data that this application knows about them and the store
      // does not know.
      const saved = await api.setUserStateMany(
        rows
          .filter((row) => selected.has(row.game_id))
          .map((row) => ({
            gameId: row.game_id,
            status: status || null,
            rating: row.rating,
            notes: row.notes,
          })),
      );
      onClear();
      onSaved(saved);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="bulk">
      <strong>
        {count} selected
      </strong>

      <label className="bulk-field" htmlFor="bulk-status">
        Mark as
      </label>
      <select
        id="bulk-status"
        value={status}
        onChange={(e) => setStatus(e.target.value as PlayStatus | "")}
      >
        <option value="">Not marked</option>
        {STATUSES.map((value) => (
          <option key={value} value={value}>
            {STATUS_LABEL[value]}
          </option>
        ))}
      </select>

      <button disabled={busy} onClick={() => void apply()}>
        {busy ? "Applying…" : "Apply"}
      </button>
      <button className="link" onClick={onClear}>
        clear selection
      </button>

      {error && <p role="alert">{error}</p>}
    </div>
  );
}
