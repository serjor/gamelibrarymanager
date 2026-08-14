import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";

const CONSOLE_URL = "https://dev.twitch.tv/console/apps";

/**
 * IGDB es de Twitch, y su acuerdo de desarrollador prohíbe empotrar el client
 * secret en una aplicación de escritorio. O montas un servidor propio, o cada
 * usuario registra su aplicación. Esta pantalla es el precio de no tener
 * servidor, así que al menos lleva el enlace directo y valida al momento.
 */
export function IgdbSetup({ onConnected }: { onConnected: () => void }) {
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.setIgdbCredentials(clientId, clientSecret);
      onConnected();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <h2>Metadatos: IGDB</h2>
      <p className="hint">
        Hace falta una aplicación tuya en el portal de Twitch. Es gratis para uso
        no comercial. Sin esto la biblioteca funciona igual, pero las fichas se
        quedan con el título de la tienda y sin portada.
      </p>
      {/* El formulario de Twitch pide tres cosas que no son evidentes, y
          equivocarse en la tercera te deja sin secret y sin saber por qué. */}
      <p className="hint">
        Al registrarla, el portal te pedirá:
      </p>
      <ul className="hint">
        <li>
          <strong>OAuth Redirect URL</strong>: pon <code>http://localhost</code>.
          Es obligatorio en su formulario, pero aquí no se usa nunca: esta
          aplicación pide el token con tus credenciales, sin ninguna redirección.
        </li>
        <li>
          <strong>Client Type</strong>: <strong>Confidential</strong>. Una
          aplicación pública no llega a darte Client Secret.
        </li>
        <li>
          <strong>Name</strong>: es único en todo Twitch, así que puede que
          tengas que probar varios.
        </li>
      </ul>

      <label htmlFor="client-id">Client ID</label>
      <input
        id="client-id"
        value={clientId}
        onChange={(e) => setClientId(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        required
      />

      <label htmlFor="client-secret">Client Secret</label>
      <input
        id="client-secret"
        type="password"
        value={clientSecret}
        onChange={(e) => setClientSecret(e.target.value)}
        autoComplete="off"
        required
      />

      <button
        type="button"
        className="link"
        onClick={() => {
          openUrl(CONSOLE_URL).catch((cause: unknown) =>
            setError(`No he podido abrir ${CONSOLE_URL}: ${errorMessage(cause)}`),
          );
        }}
      >
        Registrar mi aplicación en dev.twitch.tv
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !clientId || !clientSecret}>
        {busy ? "Comprobando…" : "Guardar"}
      </button>
    </form>
  );
}
