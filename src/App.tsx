import { useCallback, useEffect, useState } from "react";
import {
  api,
  errorMessage,
  type Account,
  type AppInfo,
  type LibrarySummary,
  type SyncReport,
} from "./lib/api";
import { SteamSetup } from "./features/onboarding/SteamSetup";
import { UnlockSecrets } from "./features/onboarding/UnlockSecrets";

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [summary, setSummary] = useState<LibrarySummary | null>(null);
  const [report, setReport] = useState<SyncReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);

  // `alive` evita que una carga en vuelo escriba estado sobre un componente ya
  // desmontado, que es la carrera clásica de este patrón.
  const load = useCallback(async (alive: () => boolean) => {
    try {
      const [nextInfo, nextAccounts, nextSummary] = await Promise.all([
        api.appInfo(),
        api.listAccounts(),
        api.librarySummary(),
      ]);
      if (!alive()) return;
      setInfo(nextInfo);
      setAccounts(nextAccounts);
      setSummary(nextSummary);
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

  const sync = async () => {
    setSyncing(true);
    setError(null);
    try {
      setReport(await api.syncNow());
      await refresh();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setSyncing(false);
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
        <UnlockSecrets onUnlocked={() => void refresh()} />
      </main>
    );
  }

  if (accounts.length === 0) {
    return (
      <main>
        <SteamSetup onConnected={() => void refresh()} />
      </main>
    );
  }

  return (
    <main>
      <header>
        <h1>Biblioteca</h1>
        <button onClick={() => void sync()} disabled={syncing}>
          {syncing ? "Sincronizando…" : "Sincronizar"}
        </button>
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
          {summary.owned} en la biblioteca · {summary.wishlist} deseados
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
      <p className="hint">
        Las fichas unificadas y el backlog llegan en las fases 4 y 5. Ahora mismo
        esto solo demuestra que la biblioteca real entra en la base de datos.
      </p>
    </main>
  );
}
