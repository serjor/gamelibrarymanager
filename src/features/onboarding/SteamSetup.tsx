import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";
import { SetupFrame } from "./SetupFrame";

const KEY_URL = "https://steamcommunity.com/dev/apikey";
const ID_URL = "https://steamid.io";

/**
 * To ask for two keys before the user sees one game is the largest risk that
 * they leave the product, thus this screen carries the direct links, says what
 * each value is for and examines the values against the API immediately: you see
 * a copy-and-paste error here and not as an empty library.
 */
export function SteamSetup({
  onConnected,
  onBack,
}: {
  onConnected: () => void;
  onBack?: () => void;
}) {
  const [apiKey, setApiKey] = useState("");
  const [steamId, setSteamId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openLink = (url: string) => {
    // If the browser does not open, the user looks at a dead link and does not
    // know why: it is better to say it and show the URL. And with the reason: to
    // hide the reason turned an incorrect permission into a mystery.
    openUrl(url).catch((cause: unknown) =>
      setError(`Could not open ${url}: ${errorMessage(cause)}`),
    );
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
    <SetupFrame
      step="Step 1 · Connect a store"
      title="Bring your collection together"
      description="Connect one store to start your local game archive. You can add the other stores later."
      trust="Your Steam API key stays on this computer. The application never asks for your Steam password."
      onBack={onBack}
    >
      <form onSubmit={submit}>
      <h2>Connect Steam</h2>
      <p className="hint">
        The key is yours and it does not leave this computer. And because it is
        yours, Steam lets you read your library even if your profile is private.
      </p>

      <label htmlFor="api-key">Steam API key</label>
      <input
        id="api-key"
        value={apiKey}
        onChange={(e) => setApiKey(e.target.value)}
        placeholder="32 characters"
        autoComplete="off"
        spellCheck={false}
        required
      />
      <button type="button" className="link" onClick={() => openLink(KEY_URL)}>
        Get my key at steamcommunity.com
      </button>

      <label htmlFor="steam-id">Your 64-bit SteamID</label>
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
        Find my SteamID
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !apiKey || !steamId}>
        {busy ? "Examining the key…" : "Connect"}
      </button>
      </form>
    </SetupFrame>
  );
}
