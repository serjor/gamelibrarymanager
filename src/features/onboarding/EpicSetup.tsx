import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";

/**
 * Where the client credentials come from. Epic has no application registry
 * either: the only client its authorisation server knows is the one of its own
 * launcher, and that pair has been published in legendary for years.
 */
const LEGENDARY_EGS_URL =
  "https://github.com/legendary-gl/legendary/blob/master/legendary/api/egs.py";

/**
 * The same screen as GOG and for the same reason: the pair identifies the
 * application, not the user, and it is asked for so that the program does not
 * have to carry it inside.
 *
 * The warning about Epic is not decoration. This is the store with no public
 * contract of any kind, so it is the one that can stop working from one day to
 * the next, and saying so before the user connects is cheaper than explaining
 * it afterwards.
 */
export function EpicSetup({ onConnected }: { onConnected: () => void }) {
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.connectEpic(clientId, clientSecret);
      onConnected();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <h2>Conectar Epic</h2>
      <p className="hint">
        Tu contraseña de Epic no pasa por aquí: se abrirá la página de Epic para
        que inicies sesión en ella, y esta aplicación solo recibe el permiso que
        te devuelva.
      </p>
      <p className="hint">
        Epic tampoco permite registrar aplicaciones, así que hace falta el par de
        cliente de su propio lanzador. No es una clave tuya y es la misma para
        todo el mundo; se te pide para que no vaya escrita dentro del programa.
      </p>
      <p className="hint">
        Epic no publica ninguna API de biblioteca: esto se apoya en la de su
        lanzador. Si algún día deja de funcionar, podrás desactivar Epic y el
        resto de tu biblioteca seguirá igual.
      </p>

      <label htmlFor="epic-client-id">Client ID</label>
      <input
        id="epic-client-id"
        value={clientId}
        onChange={(e) => setClientId(e.target.value)}
        placeholder="32 caracteres"
        autoComplete="off"
        spellCheck={false}
        required
      />

      <label htmlFor="epic-client-secret">Client secret</label>
      <input
        id="epic-client-secret"
        value={clientSecret}
        onChange={(e) => setClientSecret(e.target.value)}
        placeholder="32 caracteres"
        autoComplete="off"
        spellCheck={false}
        required
      />
      <button
        type="button"
        className="link"
        onClick={() => {
          openUrl(LEGENDARY_EGS_URL).catch((cause: unknown) =>
            setError(`No he podido abrir ${LEGENDARY_EGS_URL}: ${errorMessage(cause)}`),
          );
        }}
      >
        Dónde encontrar el par del lanzador de Epic
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !clientId || !clientSecret}>
        {busy ? "Esperando a que inicies sesión…" : "Iniciar sesión en Epic"}
      </button>
    </form>
  );
}
