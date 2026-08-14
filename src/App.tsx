import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AppInfo {
  version: string;
  stores: string[];
}

/**
 * Pantalla mínima de la fase 1: solo demuestra que el puente UI↔Rust responde.
 * La biblioteca llega en la fase 5.
 */
export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info")
      .then(setInfo)
      .catch((cause: unknown) => setError(String(cause)));
  }, []);

  return (
    <main>
      <h1>Game Library Manager</h1>
      {error && <p role="alert">{error}</p>}
      {info && (
        <p>
          v{info.version} · conectores: {info.stores.join(", ")}
        </p>
      )}
    </main>
  );
}
