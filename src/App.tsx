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
import { SetupLoading } from "./features/onboarding/SetupFrame";
import { ReviewQueue } from "./features/review/ReviewQueue";
import { Library, type View } from "./features/library/Library";
import { Today } from "./features/today/Today";
import { ActivityStrip } from "./features/shell/ActivityStrip";
import { AppShell, type AppTab } from "./features/shell/AppShell";
import { useThemePreference } from "./features/shell/theme";
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
  const { theme, onThemeChange } = useThemePreference();
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
  const [tab, setTab] = useState<AppTab>("library");
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
  const [stoppedPass, setStoppedPass] = useState<string | null>(null);
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
    setStoppedPass(null);
    setReport(null);
    try {
      const result = await action();
      if (label === "sync") {
        const syncReport = result as SyncReport;
        setReport(syncReport);
        if (syncReport.cancelled) {
          setStoppedPass(
            'The synchronisation stopped. Work made to that point is kept; click "Synchronise" again to continue.',
          );
        }
      }
      // A pass that stopped in the middle does not give an error: it keeps its
      // work and gives back the reason. If the interface did not say that here,
      // the user would see incomplete work with no word about why, which is
      // worse than an error.
      if (label === "identity") {
        const { stopped } = result as IdentityReport;
        if (stopped !== null) {
          setStoppedPass(
            `The matching stopped: ${stopped}. Work made to that point is kept; click "Match" again to continue from there.`,
          );
        }
      }
      if (
        label === "prices" &&
        (result as { cancelled?: boolean }).cancelled === true
      ) {
        setStoppedPass(
          'The price update stopped. Work made to that point is kept; click "Update the prices" again to continue.',
        );
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

  const retryOpening = () => {
    setError(null);
    void refresh();
  };
  if (!info) {
    return (
      <main className="setup-page">
        <SetupLoading error={error} onRetry={retryOpening} />
      </main>
    );
  }

  // With no open store you cannot keep a key, thus it is the first thing to
  // resolve.
  if (!info.unlocked) {
    return (
      <main className="setup-page">
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
      <main className="setup-page">
        {setup === "steam" && (
          <SteamSetup onConnected={closeSetup} onBack={() => setSetup(null)} />
        )}
        {setup === "gog" && (
          <GogSetup onConnected={closeSetup} onBack={() => setSetup(null)} />
        )}
        {setup === "epic" && (
          <EpicSetup onConnected={closeSetup} onBack={() => setSetup(null)} />
        )}
        {setup === "igdb" && (
          <IgdbSetup onConnected={closeSetup} onBack={() => setSetup(null)} />
        )}
        {setup === "itad" && (
          <ItadSetup onConnected={closeSetup} onBack={() => setSetup(null)} />
        )}
      </main>
    );
  }

  // You must start at some store, and Steam is the only store with an official
  // method. But a user with no Steam cannot be in a dead end, and that applies
  // to all of the other stores: the list comes from STORES so that a new store
  // does not forget this screen.
  if (accounts.length === 0) {
    return (
      <main className="setup-page">
        <SteamSetup onConnected={refresh} />
        <section
          className="setup-alternatives"
          aria-labelledby="setup-alternatives-title"
        >
          <p id="setup-alternatives-title" className="setup-alternatives-title">
            Or start with another store
          </p>
          <div className="setup-alternatives-actions">
            {STORES.filter(([store]) => store !== "steam").map(([store, name]) => (
              <button
                key={store}
                type="button"
                className="link"
                onClick={() => setSetup(store)}
              >
                or start with {name}
              </button>
            ))}
          </div>
        </section>
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
    <AppShell
      tab={tab}
      onTabChange={setTab}
      wishlistCount={wished}
      reviewCount={queue.length}
      summary={summary}
      activity={
        <ActivityStrip
          operation={busy}
          progress={progress}
          error={error}
          report={report}
          stoppedPass={stoppedPass}
          providerProblems={withProblem}
          exportedPath={exportedPath}
          storeName={nameOf}
          onCancel={() => void api.cancelOperation()}
        />
      }
      utility={{
        accounts,
        missingStores: toConnect.map(([id, name]) => ({ id, name })),
        connectors,
        hasIgdb,
        hasItad,
        busy,
        theme,
        onThemeChange,
        summary,
        storeName: nameOf,
        onSetup: (target) => setSetup(target),
        onSync: () => void run("sync", api.syncNow),
        onMatch: () => void run("identity", api.resolveIdentities),
        onExport: (format) => void exportLibrary(format),
        onDisconnect: disconnect,
        onToggleConnector: (connector) =>
          void run("connector", () =>
            api.setConnectorEnabled(connector.store, !connector.enabled),
          ),
      }}
    >
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
    </AppShell>
  );
}
