import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";

/**
 * De dónde salen las credenciales de cliente. GOG no tiene registro de
 * aplicaciones: el único cliente que su servidor reconoce es el de GOG Galaxy,
 * y su par está publicado desde hace años en gogdl, que es software libre.
 */
const GOGDL_AUTH_URL =
  "https://github.com/Heroic-Games-Launcher/heroic-gogdl/blob/main/gogdl/auth.py";

/**
 * A diferencia de Steam, aquí no se pide ninguna clave *personal*: el par
 * identifica a la aplicación, no al usuario. Se le pide de todos modos para que
 * el programa no tenga que llevarlo dentro, que es la única razón de que esta
 * pantalla exista y conviene decirla sin rodeos.
 */
export function GogSetup({ onConnected }: { onConnected: () => void }) {
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.connectGog(clientId, clientSecret);
      onConnected();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <h2>Conectar GOG</h2>
      <p className="hint">
        Tu contraseña de GOG no pasa por aquí: se abrirá la página de GOG para
        que inicies sesión en ella, y esta aplicación solo recibe el permiso que
        te devuelva.
      </p>
      <p className="hint">
        GOG no permite registrar aplicaciones, así que hace falta el par de
        cliente de GOG Galaxy. No es una clave tuya y es la misma para todo el
        mundo; se te pide para que no vaya escrita dentro del programa.
      </p>

      <label htmlFor="gog-client-id">Client ID</label>
      <input
        id="gog-client-id"
        value={clientId}
        onChange={(e) => setClientId(e.target.value)}
        placeholder="46899977096215655"
        autoComplete="off"
        spellCheck={false}
        required
      />

      <label htmlFor="gog-client-secret">Client secret</label>
      <input
        id="gog-client-secret"
        value={clientSecret}
        onChange={(e) => setClientSecret(e.target.value)}
        placeholder="64 caracteres"
        autoComplete="off"
        spellCheck={false}
        required
      />
      <button
        type="button"
        className="link"
        onClick={() => {
          openUrl(GOGDL_AUTH_URL).catch((cause: unknown) =>
            setError(`No he podido abrir ${GOGDL_AUTH_URL}: ${errorMessage(cause)}`),
          );
        }}
      >
        Dónde encontrar el par de GOG Galaxy
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !clientId || !clientSecret}>
        {busy ? "Esperando a que inicies sesión…" : "Iniciar sesión en GOG"}
      </button>
    </form>
  );
}
