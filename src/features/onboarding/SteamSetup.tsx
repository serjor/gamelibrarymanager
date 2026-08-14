import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";

const KEY_URL = "https://steamcommunity.com/dev/apikey";
const ID_URL = "https://steamid.io";

/**
 * Pedir dos claves antes de ver un solo juego es el mayor riesgo de abandono
 * del producto, así que el asistente lleva los enlaces directos, explica para
 * qué sirve cada dato y valida contra la API en el momento: un error de copiar
 * y pegar se ve aquí y no como una biblioteca vacía.
 */
export function SteamSetup({ onConnected }: { onConnected: () => void }) {
  const [apiKey, setApiKey] = useState("");
  const [steamId, setSteamId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openLink = (url: string) => {
    // Si el navegador no se abre, el usuario se queda mirando un enlace muerto
    // sin saber por qué: mejor decirlo y dejar la URL a la vista.
    openUrl(url).catch(() => setError(`No he podido abrir ${url}`));
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.connectSteam(apiKey, steamId);
      onConnected();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <h2>Conectar Steam</h2>
      <p className="hint">
        La clave es tuya y no sale de este ordenador. Al ser tuya, además, Steam
        te deja leer tu biblioteca aunque tengas el perfil en privado.
      </p>

      <label htmlFor="api-key">Clave de API de Steam</label>
      <input
        id="api-key"
        value={apiKey}
        onChange={(e) => setApiKey(e.target.value)}
        placeholder="32 caracteres"
        autoComplete="off"
        spellCheck={false}
        required
      />
      <button type="button" className="link" onClick={() => openLink(KEY_URL)}>
        Sacar mi clave en steamcommunity.com
      </button>

      <label htmlFor="steam-id">Tu SteamID de 64 bits</label>
      <input
        id="steam-id"
        value={steamId}
        onChange={(e) => setSteamId(e.target.value)}
        placeholder="7656119…"
        inputMode="numeric"
        autoComplete="off"
        required
      />
      <button type="button" className="link" onClick={() => openLink(ID_URL)}>
        Averiguar mi SteamID
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !apiKey || !steamId}>
        {busy ? "Comprobando la clave…" : "Conectar"}
      </button>
    </form>
  );
}
