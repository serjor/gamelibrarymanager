import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import {
  api,
  errorMessage,
  type Account,
  type AppInfo,
  type ConnectorState,
  type ExportFormat,
  type IdentityReport,
  type LibraryRow,
  type LibrarySummary,
  type PriceRow,
  type ReviewItem,
  type SyncProgress,
  type SyncReport,
} from "./lib/api";
import { SteamSetup } from "./features/onboarding/SteamSetup";
import { GogSetup } from "./features/onboarding/GogSetup";
import { EpicSetup } from "./features/onboarding/EpicSetup";
import { IgdbSetup } from "./features/onboarding/IgdbSetup";
import { ItadSetup } from "./features/onboarding/ItadSetup";
import { UnlockSecrets } from "./features/onboarding/UnlockSecrets";
import { ReviewQueue } from "./features/review/ReviewQueue";
import { Library, type View } from "./features/library/Library";
import { Today } from "./features/today/Today";
import { BrandMark } from "./features/shell/BrandMark";
import { Wishlist } from "./features/wishlist/Wishlist";

/** The stores that the application can read, with the name that it shows. */
const STORES = [
  ["steam", "Steam"],
  ["gog", "GOG"],
  ["epic", "Epic"],
] as const;

type Store = (typeof STORES)[number][0];

function nameOf(store: string): string {
  return STORES.find(([id]) => id === store)?.[1] ?? store;
}

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [connectors, setConnectors] = useState<ConnectorState[]>([]);
  const [hasIgdb, setHasIgdb] = useState(false);
  const [hasItad, setHasItad] = useState(false);
  const [summary, setSummary] = useState<LibrarySummary | null>(null);
  const [queue, setQueue] = useState<ReviewItem[]>([]);
  const [rows, setRows] = useState<LibraryRow[]>([]);
  const [prices, setPrices] = useState<PriceRow[]>([]);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  // It starts at the library and not at "Today": if the recommendation is
  // incorrect, it must not be the first thing that you see each day.
  const [tab, setTab] = useState<"library" | "today" | "wishlist" | "review">("library");
  // The view mode lives here and not in the library, because a change of tab
  // removes the library: if the library kept the mode, a return from "To review"
  // would always give you the table even if you were looking at the covers.
  const [view, setView] = useState<View>("table");
  // A setup screen opened on top of the library. None of them blocks the
  // application: you go to them when you want.
  const [setup, setSetup] = useState<Store | "igdb" | "itad" | null>(null);
  const [report, setReport] = useState<SyncReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [exportedPath, setExportedPath] = useState<string | null>(null);

  // `alive` prevents a load in progress from writing state on a component that
  // is already removed, which is the usual race of this pattern.
  const load = useCallback(async (alive: () => boolean) => {
    try {
      const nextInfo = await api.appInfo();
      if (!alive()) return;
      setInfo(nextInfo);
      if (!nextInfo.unlocked) return;

      const [
        nextAccounts,
        nextConnectors,
        nextIgdb,
        nextItad,
        nextSummary,
        nextQueue,
        nextRows,
        nextPrices,
      ] = await Promise.all([
        api.listAccounts(),
        api.connectorStates(),
        api.hasIgdbCredentials(),
        api.hasItadCredentials(),
        api.librarySummary(),
        api.reviewQueue(),
        api.library(),
        api.prices(),
      ]);
      if (!alive()) return;
      setAccounts(nextAccounts);
      setConnectors(nextConnectors);
      setHasIgdb(nextIgdb);
      setHasItad(nextItad);
      setSummary(nextSummary);
      setQueue(nextQueue);
      setRows(nextRows);
      setPrices(nextPrices);
    } catch (cause) {
      if (alive()) setError(errorMessage(cause));
    }
  }, []);

  const refresh = useCallback(() => load(() => true), [load]);

  /**
   * The rows that a save gave back replace those same games, and nothing else
   * is asked for.
   *
   * A save changes one record. Before, the answer was `refresh()`: eight
   * commands, all of the library, all of the review queue and all of the
   * prices, and the list jumped back to the top. `refresh()` stays where it
   * belongs — the synchronisation, the matching and the prices do change the
   * queue and the summary — and a save no longer goes through it.
   */
  const patchRows = useCallback((saved: LibraryRow[]) => {
    if (saved.length === 0) return;
    const byId = new Map(saved.map((row) => [row.game_id, row]));
    setRows((previous) => previous.map((row) => byId.get(row.game_id) ?? row));
  }, []);

  useEffect(() => {
    let mounted = true;
    // The rule marks every setState that an effect can reach. Here the load is
    // asynchronous and writes only if the component is still mounted, which is
    // the condition that the rule does not tell apart. It will go away in phase
    // 5, when a data library loads the library.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void load(() => mounted);
    return () => {
      mounted = false;
    };
  }, [load]);

  // The progress comes through events from Rust: the window does not stay quiet
  // while one thousand games synchronise.
  useEffect(() => {
    const unlisten = listen<SyncProgress>("sync:progress", (event) =>
      setProgress(event.payload),
    );
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  const run = async (label: string, action: () => Promise<unknown>) => {
    setBusy(label);
    setError(null);
    try {
      const result = await action();
      if (label === "sync") setReport(result as SyncReport);
      // A pass that stopped in the middle does not give an error: it keeps its
      // work and gives back the reason. If the interface did not say that here,
      // the user would see incomplete work with no word about why, which is
      // worse than an error.
      if (label === "identity") {
        const { stopped } = result as IdentityReport;
        if (stopped !== null) {
          setError(
            `The matching stopped: ${stopped}. The work made to that point is ` +
              'kept; click "Match" again to continue from there.',
          );
        }
      }
      await refresh();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
      setProgress(null);
    }
  };

  const disconnect = (account: Account) => {
    const name = nameOf(account.store);
    if (!window.confirm(`Disconnect ${name}? The records and your notes stay in the library.`)) {
      return;
    }
    void run("disconnect", () => api.disconnectAccount(account.store, account.account_ref));
  };

  const exportLibrary = async (format: ExportFormat) => {
    setBusy(`export-${format}`);
    setError(null);
    try {
      const path = await save({
        defaultPath: `game-library.${format}`,
        filters: [{ name: `${format.toUpperCase()} files`, extensions: [format] }],
      });
      if (path === null) return;
      await api.exportLibrary(path, format);
      setExportedPath(path);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  if (!info) {
    return (
      <main>
        <p>{error ?? "Opening the library…"}</p>
      </main>
    );
  }

  // With no open store you cannot keep a key, thus it is the first thing to
  // resolve.
  if (!info.unlocked) {
    return (
      <main>
        <UnlockSecrets onUnlocked={refresh} />
      </main>
    );
  }

  const closeSetup = () => {
    setSetup(null);
    refresh();
  };

  if (setup !== null) {
    return (
      <main>
        {setup === "steam" && <SteamSetup onConnected={closeSetup} />}
        {setup === "gog" && <GogSetup onConnected={closeSetup} />}
        {setup === "epic" && <EpicSetup onConnected={closeSetup} />}
        {setup === "igdb" && <IgdbSetup onConnected={closeSetup} />}
        {setup === "itad" && <ItadSetup onConnected={closeSetup} />}
        <button className="link" onClick={() => setSetup(null)}>
          Back
        </button>
      </main>
    );
  }

  // You must start at some store, and Steam is the only store with an official
  // method. But a user with no Steam cannot be in a dead end, and that applies
  // to all of the other stores: the list comes from STORES so that a new store
  // does not forget this screen.
  if (accounts.length === 0) {
    return (
      <main>
        <SteamSetup onConnected={refresh} />
        {STORES.filter(([store]) => store !== "steam").map(([store, name]) => (
          <button key={store} className="link" onClick={() => setSetup(store)}>
            or start with {name}
          </button>
        ))}
      </main>
    );
  }

  // Each store that is absent must stay reachable from here. With the first
  // screen alone, a user who started with GOG had no way to add Steam later.
  const connected = new Set(accounts.map((account) => account.store));
  const toConnect = STORES.filter(([store]) => !connected.has(store));

  // Only the connectors that have something to say. A store that operates
  // correctly does not appear: its row does not even exist until something
  // occurs.
  const withProblem = connectors.filter(
    (connector) => !connector.enabled || connector.last_error !== null,
  );

  // Records and not copies: the summary counts what the stores say — the same
  // wished-for game in two stores counts two times — and the tab must say the
  // same as the screen that it opens.
  const wished = rows.filter((row) => row.wishlist_stores.length > 0).length;

  return (
    <main>
      <header>
        <div className="brand-title">
          <BrandMark />
          <h1>Library</h1>
        </div>
        <div className="actions">
          <button
            className="primary-action"
            onClick={() => void run("sync", api.syncNow)}
            disabled={busy !== null}
          >
            {busy === "sync" ? "Synchronising…" : "Synchronise"}
          </button>
          {busy !== null && (
            <button className="link" onClick={() => void api.cancelOperation()}>
              cancel
            </button>
          )}
          <button
            onClick={() => void run("identity", api.resolveIdentities)}
            disabled={busy !== null}
          >
            {busy === "identity" ? "Matching…" : "Match"}
          </button>
          {toConnect.map(([store, name]) => (
            <button key={store} className="link" onClick={() => setSetup(store)}>
              Connect {name}
            </button>
          ))}
        </div>
      </header>

      {/* With no IGDB the library operates, but the records come from the title
          of the store. It is correct to say that, and not as an error: it is
          not an error. */}
      {!hasIgdb && (
        <p className="hint">
          No metadata: the records are made with the title of the store and with
          no cover.{" "}
          <button className="link" onClick={() => setSetup("igdb")}>
            Configure IGDB
          </button>
        </p>
      )}

      <ul className="accounts">
        {accounts.map((account) => (
          <li key={`${account.store}:${account.account_ref}`}>
            <strong>{account.store}</strong> · {account.display_name ?? account.account_ref}
            {account.last_sync_at === null
              ? " · not synchronised"
              : ` · ${new Date(account.last_sync_at * 1000).toLocaleString()}`} {" "}
            <button
              className="link"
              disabled={busy !== null}
              onClick={() => disconnect(account)}
            >
              Disconnect {nameOf(account.store)}
            </button>
          </li>
        ))}
      </ul>

      {/* A store that is broken cannot make the application useless. The
          interface says what occurs to it and offers to switch it off, which is
          what keeps the remainder unchanged. */}
      {withProblem.length > 0 && (
        <ul className="connectors">
          {withProblem.map((connector) => (
            <li key={connector.store}>
              <strong>{nameOf(connector.store)}</strong>{" "}
              {connector.enabled
                ? `could not synchronise: ${connector.last_error}`
                : "is switched off: it does not synchronise, and the data that it gave stays in the library."}{" "}
              <button
                className="link"
                disabled={busy !== null}
                onClick={() =>
                  void run("connector", () =>
                    api.setConnectorEnabled(connector.store, !connector.enabled),
                  )
                }
              >
                {connector.enabled
                  ? `Switch ${nameOf(connector.store)} off`
                  : `Switch ${nameOf(connector.store)} on`}
              </button>
            </li>
          ))}
        </ul>
      )}

      {summary && (
        <p className="summary">
          {summary.games} records · {summary.owned} owned copies · {summary.wishlist} wished for
          {summary.pending_review > 0 && ` · ${summary.pending_review} to review`}
        </p>
      )}

      {report && report.failures.length > 0 && (
        <ul role="alert">
          {report.failures.map((failure) => (
            <li key={`${failure.store}:${failure.account}`}>
              {failure.store}: {failure.reason}
            </li>
          ))}
        </ul>
      )}

      {progress && progress.total > 0 && (
        <p className="hint">
          {/* To match one thousand games takes minutes because of the IGDB
              limit: with no bar, the user cannot tell "it is slow" from "it has
              stopped". */}
          <progress value={progress.done} max={progress.total} />{" "}
          {progress.stage} · {progress.done} of {progress.total} (
          {Math.floor((progress.done / progress.total) * 100)}%)
        </p>
      )}

      {error && <p role="alert">{error}</p>}

      <nav className="tabs">
        <button
          className={tab === "library" ? "tab active" : "tab"}
          onClick={() => setTab("library")}
        >
          Library
        </button>
        <button
          className={tab === "today" ? "tab active" : "tab"}
          onClick={() => setTab("today")}
        >
          Today
        </button>
        <button
          className={tab === "wishlist" ? "tab active" : "tab"}
          onClick={() => setTab("wishlist")}
        >
          Wishlist{wished > 0 && ` (${wished})`}
        </button>
        <button
          className={tab === "review" ? "tab active" : "tab"}
          onClick={() => setTab("review")}
        >
          To review{queue.length > 0 && ` (${queue.length})`}
        </button>
        <span className="exports">
          <button
            className="link"
            disabled={busy !== null}
            onClick={() => void exportLibrary("json")}
          >
            {busy === "export-json" ? "Exporting JSON…" : "Export JSON"}
          </button>
          <button
            className="link"
            disabled={busy !== null}
            onClick={() => void exportLibrary("csv")}
          >
            {busy === "export-csv" ? "Exporting CSV…" : "Export CSV"}
          </button>
        </span>
      </nav>

      {exportedPath && <p className="hint export-path">Written to {exportedPath}</p>}

      {tab === "library" && (
        <Library rows={rows} view={view} onView={setView} onSaved={patchRows} />
      )}
      {tab === "today" && <Today rows={rows} onSaved={patchRows} />}
      {tab === "wishlist" && (
        <Wishlist
          rows={rows}
          prices={prices}
          copies={summary?.wishlist ?? 0}
          hasItad={hasItad}
          busy={busy !== null}
          onRefresh={() => void run("prices", api.refreshPrices)}
          onSetup={() => setSetup("itad")}
        />
      )}
      {tab === "review" && <ReviewQueue items={queue} onResolved={refresh} />}
    </main>
  );
}
