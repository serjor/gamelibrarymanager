import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";

const CONSOLE_URL = "https://dev.twitch.tv/console/apps";

/**
 * IGDB belongs to Twitch, and its developer agreement prohibits a client secret
 * inside a desktop application. Either you build a server of your own, or each
 * user registers their application. This screen is the cost of no server, thus
 * it at least carries the direct link and examines the credentials immediately.
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
      <h2>Metadata: IGDB</h2>
      <p className="hint">
        An application of your own in the Twitch portal is necessary. It is free
        for non-commercial use. Without this the library operates in the same
        way, but the records keep the title of the store and have no cover.
      </p>
      {/* The Twitch form asks for three things that are not clear, and a mistake
          in the third one leaves you with no secret and with no reason why. */}
      <p className="hint">
        When you register it, the portal will ask you for:
      </p>
      <ul className="hint">
        <li>
          <strong>OAuth Redirect URL</strong>: write <code>http://localhost</code>.
          Their form makes it necessary, but this application never uses it: this
          application asks for the token with your credentials and with no
          redirect.
        </li>
        <li>
          <strong>Client Type</strong>: <strong>Confidential</strong>. A public
          application does not give you a Client Secret.
        </li>
        <li>
          <strong>Name</strong>: it is unique in all of Twitch, thus you can have
          to try more than one.
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
            setError(`Could not open ${CONSOLE_URL}: ${errorMessage(cause)}`),
          );
        }}
      >
        Register my application in dev.twitch.tv
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !clientId || !clientSecret}>
        {busy ? "Examining…" : "Save"}
      </button>
    </form>
  );
}
