import { useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { SetupFrame } from "./SetupFrame";

/**
 * It appears only on machines with no keyring: containers, minimal desktops,
 * remote sessions. To find that when you keep the first key would be the worst
 * moment to find it, thus the application detects it at the start.
 */
export function UnlockSecrets({
  onUnlocked,
  onBack,
}: {
  onUnlocked: () => void;
  onBack?: () => void;
}) {
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
    <SetupFrame
      step="Before first use · Local security"
      title="Unlock your local secret store"
      description="Enter the passphrase that protects credentials on this machine."
      trust="The passphrase decrypts local credentials. It is never sent to a store and the application does not keep a copy."
      onBack={onBack}
    >
      <form onSubmit={submit}>
      <h2>Passphrase of the store</h2>
      <p className="hint">
        This system has no keyring in which to keep the keys, thus they are
        encrypted in a file. If you lose this passphrase you must write the API
        keys again: the passphrase is kept in no place.
      </p>

      <label htmlFor="passphrase">Passphrase</label>
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
        {busy ? "Opening…" : "Open the store"}
      </button>
      </form>
    </SetupFrame>
  );
}
