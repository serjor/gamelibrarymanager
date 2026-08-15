import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  errorMessage,
  type Account,
  type AppInfo,
  type ConnectorState,
  type LibraryRow,
  type LibrarySummary,
  type ReviewItem,
  type SyncProgress,
  type SyncReport,
} from "./lib/api";
import { SteamSetup } from "./features/onboarding/SteamSetup";
import { GogSetup } from "./features/onboarding/GogSetup";
import { EpicSetup } from "./features/onboarding/EpicSetup";
import { IgdbSetup } from "./features/onboarding/IgdbSetup";
import { UnlockSecrets } from "./features/onboarding/UnlockSecrets";
import { ReviewQueue } from "./features/review/ReviewQueue";
import { Library, type Vista } from "./features/library/Library";
import { Today } from "./features/today/Today";

/** Las tiendas que se saben leer, con el nombre que se enseña de cada una. */
const TIENDAS = [
  ["steam", "Steam"],
  ["gog", "GOG"],
  ["epic", "Epic"],
] as const;

type Tienda = (typeof TIENDAS)[number][0];

function nombreDe(store: string): string {
  return TIENDAS.find(([id]) => id === store)?.[1] ?? store;
}

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [connectors, setConnectors] = useState<ConnectorState[]>([]);
  const [hasIgdb, setHasIgdb] = useState(false);
  const [summary, setSummary] = useState<LibrarySummary | null>(null);
  const [queue, setQueue] = useState<ReviewItem[]>([]);
  const [rows, setRows] = useState<LibraryRow[]>([]);
  const [progress, setProgress] = useState<SyncProgress | null>(null);
  // Arranca en la biblioteca y no en «Hoy»: si la recomendación falla, no
  // conviene que sea lo primero que ves cada día.
  const [tab, setTab] = useState<"library" | "today" | "review">("library");
  // El modo de vista vive aquí y no dentro de la biblioteca porque cambiar de
  // pestaña la desmonta: si lo guardara ella, volver de «Por revisar» te
  // devolvería siempre a la tabla aunque estuvieras mirando las portadas.
  const [vista, setVista] = useState<Vista>("tabla");
  // Asistente abierto por encima de la biblioteca. Ninguno bloquea la
  // aplicación: se entra a ellos cuando el usuario quiere.
  const [setup, setSetup] = useState<Tienda | "igdb" | null>(null);
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

      const [nextAccounts, nextConnectors, nextIgdb, nextSummary, nextQueue, nextRows] =
        await Promise.all([
          api.listAccounts(),
          api.connectorStates(),
          api.hasIgdbCredentials(),
          api.librarySummary(),
          api.reviewQueue(),
          api.library(),
        ]);
      if (!alive()) return;
      setAccounts(nextAccounts);
      setConnectors(nextConnectors);
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

  if (setup !== null) {
    return (
      <main>
        {setup === "steam" && <SteamSetup onConnected={cerrarSetup} />}
        {setup === "gog" && <GogSetup onConnected={cerrarSetup} />}
        {setup === "epic" && <EpicSetup onConnected={cerrarSetup} />}
        {setup === "igdb" && <IgdbSetup onConnected={cerrarSetup} />}
        <button className="link" onClick={() => setSetup(null)}>
          Volver
        </button>
      </main>
    );
  }

  // Hay que empezar por algún sitio, y Steam es la única tienda con una vía
  // oficial. Pero quien no tenga Steam no puede quedarse en un callejón, y eso
  // vale para todas las demás: la lista sale de TIENDAS para que añadir una no
  // se olvide de esta pantalla.
  if (accounts.length === 0) {
    return (
      <main>
        <SteamSetup onConnected={refresh} />
        {TIENDAS.filter(([store]) => store !== "steam").map(([store, nombre]) => (
          <button key={store} className="link" onClick={() => setSetup(store)}>
            o empezar por {nombre}
          </button>
        ))}
      </main>
    );
  }

  // Cada tienda que falte tiene que seguir siendo alcanzable desde aquí. Con
  // solo la primera pantalla, quien empezara por GOG se quedaba sin ninguna
  // forma de añadir Steam después.
  const conectadas = new Set(accounts.map((account) => account.store));
  const porConectar = TIENDAS.filter(([store]) => !conectadas.has(store));

  // Solo los conectores que tienen algo que decir. Una tienda que va bien no
  // sale: la fila ni siquiera existe hasta que pasa algo.
  const conProblema = connectors.filter(
    (conector) => !conector.enabled || conector.last_error !== null,
  );

  return (
    <main>
      <header>
        <h1>Biblioteca</h1>
        <div className="actions">
          <button onClick={() => void run("sync", api.syncNow)} disabled={busy !== null}>
            {busy === "sync" ? "Sincronizando…" : "Sincronizar"}
          </button>
          {busy !== null && (
            <button className="link" onClick={() => void api.cancelOperation()}>
              cancelar
            </button>
          )}
          <button
            onClick={() => void run("identity", api.resolveIdentities)}
            disabled={busy !== null}
          >
            {busy === "identity" ? "Emparejando…" : "Emparejar"}
          </button>
          {porConectar.map(([store, nombre]) => (
            <button key={store} className="link" onClick={() => setSetup(store)}>
              Conectar {nombre}
            </button>
          ))}
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

      {/* Una tienda rota no puede volver inútil la aplicación. Se dice qué le
          pasa y se ofrece apagarla, que es lo que deja el resto intacto. */}
      {conProblema.length > 0 && (
        <ul className="connectors">
          {conProblema.map((conector) => (
            <li key={conector.store}>
              <strong>{nombreDe(conector.store)}</strong>{" "}
              {conector.enabled
                ? `no pudo sincronizar: ${conector.last_error}`
                : "está desactivado: no se sincroniza, y lo que ya trajo sigue en la biblioteca."}{" "}
              <button
                className="link"
                disabled={busy !== null}
                onClick={() =>
                  void run("connector", () =>
                    api.setConnectorEnabled(conector.store, !conector.enabled),
                  )
                }
              >
                {conector.enabled
                  ? `Desactivar ${nombreDe(conector.store)}`
                  : `Reactivar ${nombreDe(conector.store)}`}
              </button>
            </li>
          ))}
        </ul>
      )}

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

      {progress && progress.total > 0 && (
        <p className="hint">
          {/* Emparejar mil juegos son minutos por el límite de IGDB: sin una
              barra, el usuario no distingue «va despacio» de «se ha colgado». */}
          <progress value={progress.done} max={progress.total} />{" "}
          {progress.stage} · {progress.done} de {progress.total} (
          {Math.floor((progress.done / progress.total) * 100)}%)
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
          className={tab === "today" ? "tab active" : "tab"}
          onClick={() => setTab("today")}
        >
          Hoy
        </button>
        <button
          className={tab === "review" ? "tab active" : "tab"}
          onClick={() => setTab("review")}
        >
          Por revisar{queue.length > 0 && ` (${queue.length})`}
        </button>
      </nav>

      {tab === "library" && (
        <Library rows={rows} vista={vista} onVista={setVista} onSaved={refresh} />
      )}
      {tab === "today" && <Today rows={rows} onSaved={refresh} />}
      {tab === "review" && <ReviewQueue items={queue} onResolved={refresh} />}
    </main>
  );
}
