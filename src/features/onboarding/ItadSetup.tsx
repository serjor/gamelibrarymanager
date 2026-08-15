import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";

const APPS_URL = "https://isthereanydeal.com/apps/";

/**
 * The country with which the prices are requested, taken from the language of
 * the system.
 *
 * It is a proposal, not a rule: the field stays editable because a user whose
 * system is in English does not necessarily buy in the United States, and a user
 * whose system is in Spanish does not necessarily buy in Spain.
 */
function systemCountry(): string {
  const region = navigator.language.split("-")[1];
  return region === undefined ? "" : region.toUpperCase();
}

/**
 * Prices: ITAD.
 *
 * One more key, and for the same reason as the others: the application carries
 * no secret inside. Here the key is free and you get it in one minute, thus the
 * screen is short; what is not clear is the country, and thus it has a field of
 * its own and the application does not assume it.
 */
export function ItadSetup({ onConnected }: { onConnected: () => void }) {
  const [key, setKey] = useState("");
  const [country, setCountry] = useState(systemCountry);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.setItadCredentials(key, country);
      onConnected();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <h2>Prices: IsThereAnyDeal</h2>
      <p className="hint">
        It gives a price to your wishlist: what each store asks today and the
        lowest price that the game has had. Without this the list operates in the
        same way, only with no prices.
      </p>
      <p className="hint">
        Register in ITAD, go to your applications and make one: the key appears
        there and it is free.
      </p>

      <label htmlFor="itad-key">ITAD API key</label>
      <input
        id="itad-key"
        type="password"
        value={key}
        onChange={(e) => setKey(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        required
      />

      <label htmlFor="itad-country">Country</label>
      <input
        id="itad-country"
        value={country}
        onChange={(e) => setCountry(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        maxLength={2}
        required
      />
      {/* With no country, ITAD answers with the stores and the currency of a
          different market, and you do not see the error: you see prices, only
          they are not your prices. */}
      <p className="hint">
        A code of two letters. It decides which stores the search uses and the
        currency of the price.
      </p>

      <button
        type="button"
        className="link"
        onClick={() => {
          openUrl(APPS_URL).catch((cause: unknown) =>
            setError(`Could not open ${APPS_URL}: ${errorMessage(cause)}`),
          );
        }}
      >
        Get my key at isthereanydeal.com
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !key || !country}>
        {busy ? "Examining…" : "Save"}
      </button>
    </form>
  );
}
