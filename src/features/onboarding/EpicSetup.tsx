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
      <h2>Connect Epic</h2>
      <p className="hint">
        Your Epic password does not come through here: the Epic page will open
        for you to sign in on it, and this application receives only the
        permission that Epic gives back.
      </p>
      <p className="hint">
        Epic also does not let you register applications, thus the client pair of
        its own launcher is necessary. It is not a key of yours and it is the
        same for all users; the application asks you for it so that it is not
        written inside the program.
      </p>
      <p className="hint">
        Epic publishes no library API: this uses the API of its launcher. If it
        stops operating one day, you can switch Epic off and the remainder of
        your library will stay unchanged.
      </p>

      <label htmlFor="epic-client-id">Client ID</label>
      <input
        id="epic-client-id"
        value={clientId}
        onChange={(e) => setClientId(e.target.value)}
        placeholder="32 characters"
        autoComplete="off"
        spellCheck={false}
        required
      />

      <label htmlFor="epic-client-secret">Client secret</label>
      <input
        id="epic-client-secret"
        value={clientSecret}
        onChange={(e) => setClientSecret(e.target.value)}
        placeholder="32 characters"
        autoComplete="off"
        spellCheck={false}
        required
      />
      <button
        type="button"
        className="link"
        onClick={() => {
          openUrl(LEGENDARY_EGS_URL).catch((cause: unknown) =>
            setError(`Could not open ${LEGENDARY_EGS_URL}: ${errorMessage(cause)}`),
          );
        }}
      >
        Where to find the client pair of the Epic launcher
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !clientId || !clientSecret}>
        {busy ? "Waiting for you to sign in…" : "Sign in to Epic"}
      </button>
    </form>
  );
}
