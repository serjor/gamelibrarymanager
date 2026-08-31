import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type ReviewItem, type ScoredCandidate } from "../../lib/api";

/**
 * The base of the public IGDB records. It is written as a constant and the slug
 * is added to it, and the complete address is not interpolated, because the
 * scope of the capability is examined against the literal strings that are in
 * the code.
 */
const IGDB_GAME_URL = "https://www.igdb.com/games/";

/**
 * What the automatic matching did not decide.
 *
 * That this queue exists is the central design decision of the product: a
 * duplicate that you see is a nuisance, but two different games merged make the
 * user lose the status and the notes of one of the two, and with no message. When
 * there is doubt, the application asks.
 *
 * What almost always comes here is not doubt between different games: it is
 * equal scores between records that **are the same game** — IGDB has duplicate
 * entries, and the editions normalise to the same title. Thus they come in
 * groups and you can resolve them together: the threshold does not change,
 * because it is correct when it refuses if two different games share a name; what
 * is corrected is the work to examine them.
 *
 * That is the reason for the table. A vertical list makes you read entry by
 * entry even if one hundred and forty of the one hundred and fifty are clear; in
 * columns, you examine one column — "will match with" — and you go to the detail
 * only where something is not correct.
 */
export function ReviewQueue({ items, onResolved }: { items: ReviewItem[]; onResolved: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /**
   * What the user has touched: entry -> record, or `null` if they removed the
   * record that came selected. An entry that is not here is an entry that they
   * have not touched, and then the preselection applies. To keep only the
   * differences prevents the effect that would copy the preselection to state
   * each time that the queue loads again.
   */
  const [touched, setTouched] = useState<Record<string, number | null>>({});

  const [ties, singles] = useMemo(
    () => [items.filter((i) => i.tie), items.filter((i) => !i.tie)],
    [items],
  );

  /**
   * The entries that are not equal come with the best candidate already
   * selected; the entries that are equal come with nothing.
   *
   * That difference is all of the queue. When a candidate wins clearly, what
   * stays to do is to look whether it is the correct one and say yes, and a
   * click to repeat what the screen already says is work that nobody needs. When
   * two are equal, a selection made for the user would be exactly what the
   * threshold refused to do, and the reason that this screen exists.
   */
  const preselection = useMemo(
    () =>
      Object.fromEntries(
        singles
          .filter((item) => item.candidates.length > 0)
          .map((item) => [item.store_entry_id, item.candidates[0]!.igdb_id]),
      ) as Record<string, number>,
    [singles],
  );

  const chosen = (item: ReviewItem): number | null => {
    const value = touched[item.store_entry_id];
    return value === undefined ? (preselection[item.store_entry_id] ?? null) : value;
  };

  // A click on the candidate that is already selected removes it: it is the only
  // way to say "not this one" without you say at the same time which one, and it
  // is necessary to leave an entry out of the batch and not resolve it.
  const choose = (item: ReviewItem, igdbId: number) => {
    const current = chosen(item);
    setTouched((previous) => ({
      ...previous,
      [item.store_entry_id]: current === igdbId ? null : igdbId,
    }));
  };

  const decisions = items
    .map((item) => [item.store_entry_id, chosen(item)] as const)
    .filter((pair): pair is [string, number] => pair[1] !== null);

  const act = async (id: string, action: () => Promise<unknown>) => {
    setBusy(id);
    setError(null);
    try {
      await action();
      onResolved();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const confirmBatch = async () => {
    if (decisions.length === 0) return;
    await act("batch", async () => {
      await api.reviewConfirmMany(decisions.map(([entry, record]) => [entry, record]));
      setTouched({});
    });
  };

  if (items.length === 0) {
    return (
      <section className="review-screen command-deck empty-screen">
        <div className="empty-state" role="status">
          <strong className="empty-state-title">Review queue is clear</strong>
          <p className="hint">There is nothing to review. Synchronise a store or run Match when you have new records.</p>
        </div>
      </section>
    );
  }

  const open = (url: string) => {
    openUrl(url).catch((cause: unknown) =>
      setError(`Could not open ${url}: ${errorMessage(cause)}`),
    );
  };

  const row = (item: ReviewItem) => {
    const id = chosen(item);
    const selected = item.candidates.find((candidate) => candidate.igdb_id === id) ?? null;
    const others = item.candidates.filter((candidate) => candidate.igdb_id !== id);

    return (
      <tr key={item.store_entry_id} className={selected === null ? "not-chosen" : undefined}>
        {/* What the store says, which is what you must compare against. */}
        <td>
          <div className="source">
            {item.cover_url ? (
              <img src={item.cover_url} alt="" width={96} height={45} loading="lazy" />
            ) : (
              <span className="cover-missing wide" aria-hidden="true" />
            )}
            <strong>{item.title}</strong>
          </div>
        </td>

        <td>
          <span className="store">{item.store}</span>
          {item.store_url && (
            <button
              className="link"
              onClick={() => open(item.store_url!)}
              aria-label={`See ${item.title} in ${item.store}`}
            >
              ↗
            </button>
          )}
        </td>

        <td>
          {selected ? (
            <Candidate
              candidate={selected}
              chosen
              short
              onChoose={() => choose(item, selected.igdb_id)}
              onLook={selected.slug ? () => open(IGDB_GAME_URL + selected.slug) : undefined}
            />
          ) : (
            <span className="hint">not chosen</span>
          )}
        </td>

        <td className="num">{selected?.release_year ?? "—"}</td>
        <td className="num">{selected ? `${Math.round(selected.score * 100)}%` : "—"}</td>

        <td>
          {item.candidates.length === 0 ? (
            <p className="hint">IGDB does not know this game.</p>
          ) : (
            <ul className="candidates">
              {others.map((candidate) => (
                <li key={candidate.igdb_id}>
                  <Candidate
                    candidate={candidate}
                    chosen={false}
                    onChoose={() => choose(item, candidate.igdb_id)}
                    onLook={
                      candidate.slug ? () => open(IGDB_GAME_URL + candidate.slug) : undefined
                    }
                  />
                </li>
              ))}
            </ul>
          )}
          <button
            className="link"
            disabled={busy !== null}
            onClick={() =>
              void act(item.store_entry_id, () => api.reviewWithoutMetadata(item.store_entry_id))
            }
          >
            None: make a record with the title of the store
          </button>
        </td>
      </tr>
    );
  };

  const table = (rows: ReviewItem[]) => (
    <div className="review-viewport">
      <table className="review command-table" aria-label="Review matches">
        <colgroup>
          <col />
          <col style={{ width: "5.5rem" }} />
          <col style={{ width: "17rem" }} />
          <col style={{ width: "4rem" }} />
          <col style={{ width: "5.5rem" }} />
          <col style={{ width: "20rem" }} />
        </colgroup>
        <thead>
          <tr>
            <th>In the store</th>
            <th>Store</th>
            <th>Will match with</th>
            <th className="num">Year</th>
            <th className="num">Similarity</th>
            <th>Other IGDB records</th>
          </tr>
        </thead>
        <tbody>{rows.map(row)}</tbody>
      </table>
    </div>
  );

  return (
    <section className="review-screen command-deck">
      <h2>To review ({items.length})</h2>
      {error && <p role="alert">{error}</p>}

      {decisions.length > 0 && (
        <div className="hint sticky review-command-bar" role="region" aria-label="Review actions">
          <button
            className="primary-action"
            disabled={busy !== null}
            onClick={() => void confirmBatch()}
          >
            {busy === "batch"
              ? "Confirming…"
              : `Confirm ${decisions.length} match${decisions.length === 1 ? "" : "es"}`}
          </button>{" "}
          <button
            className="link"
            onClick={() =>
              setTouched(Object.fromEntries(items.map((item) => [item.store_entry_id, null])))
            }
          >
            clear selection
          </button>
        </div>
      )}

      {ties.length > 0 && (
        <>
          <h3>Equal scores ({ties.length})</h3>
          <p className="hint review-guidance">
            The best candidates have the same score. Almost always they are the
            same record repeated in IGDB or editions of the same game, but not
            always: two different games can have the same name, and thus the
            application does not decide alone. The cover and the year tell them
            apart.
          </p>
          {table(ties)}
        </>
      )}

      {singles.length > 0 && (
        <>
          {ties.length > 0 && <h3>The remainder ({singles.length})</h3>}
          {/* Say what will occur at a click on the batch button. A preselection
              with no words is the way to let somebody confirm one hundred and
              fifty matches while they think that they confirm only the matches
              that they touched. */}
          <p className="hint review-guidance">
            Here one candidate wins clearly, thus it comes already selected.
            Examine the "will match with" column and change what is not correct:
            nothing is written until you confirm.
          </p>
          {table(singles)}
        </>
      )}
    </section>
  );
}

/** A candidate with what is necessary to recognise it in the application. */
function Candidate({
  candidate,
  chosen,
  short,
  onChoose,
  onLook,
}: {
  candidate: ScoredCandidate;
  chosen: boolean;
  /** In the chosen column the year and the similarity have their own columns, and
   *  to repeat them there is noise. */
  short?: boolean;
  onChoose: () => void;
  /** Opens its record in IGDB. Absent when IGDB published no slug. */
  onLook?: () => void;
}) {
  return (
    <span className="candidate-wrap">
      <button
        className={chosen ? "candidate chosen" : "candidate"}
        aria-pressed={chosen}
        onClick={onChoose}
      >
        {candidate.cover_url ? (
          // Decoration: the name is already in the button, thus to repeat it in
          // the alt would only make a screen reader say it two times.
          <img src={candidate.cover_url} alt="" width={45} height={60} loading="lazy" />
        ) : (
          // A space of the same size. Without it, a candidate with no cover
          // stays as a low pill beside a high card and you can no longer read
          // the row quickly, which is exactly what the covers are for.
          <span className="cover-missing" aria-hidden="true" />
        )}
        <span>
          {candidate.name}
          {!short && candidate.release_year !== null && (
            <span className="hint"> · {candidate.release_year}</span>
          )}
          {!short && <span className="hint"> · {Math.round(candidate.score * 100)}%</span>}
        </span>
      </button>
      {onLook && (
        // A separate button and not a link inside the other one: a control inside
        // a button is not valid HTML and the keyboard would not reach the inner
        // control.
        <button
          className="link"
          onClick={onLook}
          aria-label={`See ${candidate.name} in IGDB`}
        >
          IGDB ↗
        </button>
      )}
    </span>
  );
}
