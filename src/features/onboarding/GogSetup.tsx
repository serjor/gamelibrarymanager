import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";
import { SetupFrame } from "./SetupFrame";

/**
 * Where the client credentials come from. GOG has no application registry: the
 * only client that its server knows is the client of GOG Galaxy, and its pair
 * has been published for years in gogdl, which is free software.
 */
const GOGDL_AUTH_URL =
  "https://github.com/Heroic-Games-Launcher/heroic-gogdl/blob/main/gogdl/auth.py";

/**
 * Different from Steam, this screen asks for no *personal* key: the pair
 * identifies the application, not the user. The application asks for it so that
 * the program does not have to carry it inside, which is the only reason that
 * this screen exists and it is correct to say it clearly.
 */
export function GogSetup({
  onConnected,
  onBack,
}: {
  onConnected: () => void;
  onBack?: () => void;
}) {
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
    <SetupFrame
      step="Step 1 · Connect a store"
      title="Add GOG to the archive"
      description="Use GOG's own sign-in page to bring another part of your collection into the local archive."
      trust="Your GOG password stays on GOG's page. This application receives only the permission that GOG gives back."
      onBack={onBack}
    >
      <form onSubmit={submit}>
      <h2>Connect GOG</h2>
      <p className="hint">
        Your GOG password does not come through here: the GOG page will open for
        you to sign in on it, and this application receives only the permission
        that GOG gives back.
      </p>
      <p className="hint">
        GOG does not let you register applications, thus the client pair of GOG
        Galaxy is necessary. It is not a key of yours and it is the same for all
        users; the application asks you for it so that it is not written inside
        the program.
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
        placeholder="64 characters"
        autoComplete="off"
        spellCheck={false}
        required
      />
      <button
        type="button"
        className="link"
        onClick={() => {
          openUrl(GOGDL_AUTH_URL).catch((cause: unknown) =>
            setError(`Could not open ${GOGDL_AUTH_URL}: ${errorMessage(cause)}`),
          );
        }}
      >
        Where to find the GOG Galaxy client pair
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !clientId || !clientSecret}>
        {busy ? "Waiting for you to sign in…" : "Sign in to GOG"}
      </button>
      </form>
    </SetupFrame>
  );
}
