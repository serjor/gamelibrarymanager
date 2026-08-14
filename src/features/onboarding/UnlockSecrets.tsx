import { useState } from "react";
import { api, errorMessage } from "../../lib/api";

/**
 * Solo aparece en máquinas sin keyring: contenedores, escritorios mínimos,
 * sesiones remotas. Descubrirlo al guardar la primera clave sería la peor
 * forma de enterarse, así que se detecta al arrancar.
 */
export function UnlockSecrets({ onUnlocked }: { onUnlocked: () => void }) {
  const [passphrase, setPassphrase] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.unlockSecrets(passphrase);
      onUnlocked();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <h2>Contraseña del almacén</h2>
      <p className="hint">
        Este sistema no tiene un llavero donde guardar las claves, así que se
        cifran en un fichero. Si pierdes esta contraseña habrá que volver a
        introducir las claves de API: no se guarda en ninguna parte.
      </p>

      <label htmlFor="passphrase">Contraseña</label>
      <input
        id="passphrase"
        type="password"
        value={passphrase}
        onChange={(e) => setPassphrase(e.target.value)}
        autoComplete="current-password"
        required
      />

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || passphrase.length < 8}>
        {busy ? "Abriendo…" : "Abrir almacén"}
      </button>
    </form>
  );
}
