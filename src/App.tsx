import { useCallback, useEffect, useState } from "react";
import {
  api,
  errorMessage,
  type Account,
  type AppInfo,
  type LibrarySummary,
  type ReviewItem,
  type SyncReport,
} from "./lib/api";
import { SteamSetup } from "./features/onboarding/SteamSetup";
import { IgdbSetup } from "./features/onboarding/IgdbSetup";
import { UnlockSecrets } from "./features/onboarding/UnlockSecrets";
import { ReviewQueue } from "./features/review/ReviewQueue";

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [hasIgdb, setHasIgdb] = useState(false);
  const [summary, setSummary] = useState<LibrarySummary | null>(null);
  const [queue, setQueue] = useState<ReviewItem[]>([]);
  const [report, setReport] = useState<SyncReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  // `alive` evita que una carga en vuelo escriba estado sobre un componente ya
  // desmontado, que es la carrera clásica de este patrón.
  const load = useCallback(async (alive: () => boolean) => {
    try {
      const nextInfo = await api.appInfo();
      if (!alive()) return;
      setInfo(nextInfo);
      if (!nextInfo.unlocked) return;

      const [nextAccounts, nextIgdb, nextSummary, nextQueue] = await Promise.all([
        api.listAccounts(),
        api.hasIgdbCredentials(),
        api.librarySummary(),
        api.reviewQueue(),
      ]);
      if (!alive()) return;
      setAccounts(nextAccounts);
      setHasIgdb(nextIgdb);
      setSummary(nextSummary);
      setQueue(nextQueue);
    } catch (cause) {
      if (alive()) setError(errorMessage(cause));
    }
  }, []);

  const refresh = useCallback(() => load(() => true), [load]);

  useEffect(() => {
    let mounted = true;
    // La regla marca cualquier setState alcanzable desde un efecto. Aquí la
    // carga es asíncrona y solo escribe si el componente sigue montado, que es
    // el caso que la regla no distingue. Desaparecerá en la fase 5, cuando la
    // biblioteca pase a cargarse con una librería de datos.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void load(() => mounted);
    return () => {
      mounted = false;
    };
  }, [load]);

  const run = async (label: string, action: () => Promise<unknown>) => {
    setBusy(label);
    setError(null);
    try {
      const result = await action();
      if (label === "sync") setReport(result as SyncReport);
      await refresh();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  if (!info) {
    return (
      <main>
        <p>{error ?? "Abriendo la biblioteca…"}</p>
      </main>
    );
  }

  // Sin almacén abierto no se puede guardar ninguna clave, así que es lo
  // primero que hay que resolver.
  if (!info.unlocked) {
    return (
      <main>
        <UnlockSecrets onUnlocked={refresh} />
      </main>
    );
  }

  if (accounts.length === 0) {
    return (
      <main>
        <SteamSetup onConnected={refresh} />
      </main>
    );
  }

  if (!hasIgdb) {
    return (
      <main>
        <IgdbSetup onConnected={refresh} />
      </main>
    );
  }

  return (
    <main>
      <header>
        <h1>Biblioteca</h1>
        <div className="actions">
          <button onClick={() => void run("sync", api.syncNow)} disabled={busy !== null}>
            {busy === "sync" ? "Sincronizando…" : "Sincronizar"}
          </button>
          <button
            onClick={() => void run("identity", api.resolveIdentities)}
            disabled={busy !== null}
          >
            {busy === "identity" ? "Emparejando…" : "Emparejar"}
          </button>
        </div>
      </header>

      <ul className="accounts">
        {accounts.map((account) => (
          <li key={`${account.store}:${account.account_ref}`}>
            <strong>{account.store}</strong> · {account.display_name ?? account.account_ref}
            {account.last_sync_at === null
              ? " · sin sincronizar"
              : ` · ${new Date(account.last_sync_at * 1000).toLocaleString()}`}
          </li>
        ))}
      </ul>

      {summary && (
        <p className="summary">
          {summary.games} fichas · {summary.owned} copias en propiedad · {summary.wishlist} deseados
          {summary.pending_review > 0 && ` · ${summary.pending_review} por revisar`}
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

      {error && <p role="alert">{error}</p>}

      <ReviewQueue items={queue} onResolved={refresh} />

      <p className="hint">
        La rejilla con portadas y el backlog llegan en la fase 5.
      </p>
    </main>
  );
}
