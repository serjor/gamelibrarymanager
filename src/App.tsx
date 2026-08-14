import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  errorMessage,
  type Account,
  type AppInfo,
  type LibraryRow,
  type LibrarySummary,
  type ReviewItem,
  type SyncProgress,
  type SyncReport,
} from "./lib/api";
import { SteamSetup } from "./features/onboarding/SteamSetup";
import { GogSetup } from "./features/onboarding/GogSetup";
import { IgdbSetup } from "./features/onboarding/IgdbSetup";
import { UnlockSecrets } from "./features/onboarding/UnlockSecrets";
import { ReviewQueue } from "./features/review/ReviewQueue";
import { Library } from "./features/library/Library";

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [hasIgdb, setHasIgdb] = useState(false);
  const [summary, setSummary] = useState<LibrarySummary | null>(null);
  const [queue, setQueue] = useState<ReviewItem[]>([]);
  const [rows, setRows] = useState<LibraryRow[]>([]);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  const [tab, setTab] = useState<"library" | "review">("library");
  // Asistente abierto por encima de la biblioteca. Ninguno bloquea la
  // aplicación: se entra a ellos cuando el usuario quiere.
  const [setup, setSetup] = useState<"gog" | "igdb" | null>(null);
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

      const [nextAccounts, nextIgdb, nextSummary, nextQueue, nextRows] = await Promise.all([
        api.listAccounts(),
        api.hasIgdbCredentials(),
        api.librarySummary(),
        api.reviewQueue(),
        api.library(),
      ]);
      if (!alive()) return;
      setAccounts(nextAccounts);
      setHasIgdb(nextIgdb);
      setSummary(nextSummary);
      setQueue(nextQueue);
      setRows(nextRows);
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

  // El progreso llega por eventos desde Rust: la ventana no se queda muda
  // mientras se sincronizan mil juegos.
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
      await refresh();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
      setProgress(null);
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

  const cerrarSetup = () => {
    setSetup(null);
    refresh();
  };

  if (setup === "gog") {
    return (
      <main>
        <GogSetup onConnected={cerrarSetup} />
        <button className="link" onClick={() => setSetup(null)}>
          Volver
        </button>
      </main>
    );
  }

  if (setup === "igdb") {
    return (
      <main>
        <IgdbSetup onConnected={cerrarSetup} />
        <button className="link" onClick={() => setSetup(null)}>
          Volver
        </button>
      </main>
    );
  }

  // Hay que empezar por algún sitio, y Steam es la única tienda con una vía
  // oficial. Pero quien no tenga Steam no puede quedarse en un callejón.
  if (accounts.length === 0) {
    return (
      <main>
        <SteamSetup onConnected={refresh} />
        <button className="link" onClick={() => setSetup("gog")}>
          o empezar por GOG
        </button>
      </main>
    );
  }

  const tieneGog = accounts.some((account) => account.store === "gog");

  return (
    <main>
      <header>
        <h1>Biblioteca</h1>
        <div className="actions">
          <button onClick={() => void run("sync", api.syncNow)} disabled={busy !== null}>
            {busy === "sync" ? "Sincronizando…" : "Sincronizar"}
          </button>
          {busy === "sync" && (
            <button className="link" onClick={() => void api.cancelSync()}>
              cancelar
            </button>
          )}
          <button
            onClick={() => void run("identity", api.resolveIdentities)}
            disabled={busy !== null}
          >
            {busy === "identity" ? "Emparejando…" : "Emparejar"}
          </button>
          {!tieneGog && (
            <button className="link" onClick={() => setSetup("gog")}>
              Conectar GOG
            </button>
          )}
        </div>
      </header>

      {/* Sin IGDB la biblioteca funciona, pero las fichas salen del título de
          la tienda. Conviene decirlo, y no a modo de error: no lo es. */}
      {!hasIgdb && (
        <p className="hint">
          Sin metadatos: las fichas se crean con el título de la tienda y sin
          portada.{" "}
          <button className="link" onClick={() => setSetup("igdb")}>
            Configurar IGDB
          </button>
        </p>
      )}

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

      {progress && (
        <p className="hint">
          {progress.store}: {progress.stage} ({progress.done + 1} de {progress.total})
        </p>
      )}

      {error && <p role="alert">{error}</p>}

      <nav className="tabs">
        <button
          className={tab === "library" ? "tab active" : "tab"}
          onClick={() => setTab("library")}
        >
          Biblioteca
        </button>
        <button
          className={tab === "review" ? "tab active" : "tab"}
          onClick={() => setTab("review")}
        >
          Por revisar{queue.length > 0 && ` (${queue.length})`}
        </button>
      </nav>

      {tab === "library" ? (
        <Library rows={rows} onSaved={refresh} />
      ) : (
        <ReviewQueue items={queue} onResolved={refresh} />
      )}
    </main>
  );
}
