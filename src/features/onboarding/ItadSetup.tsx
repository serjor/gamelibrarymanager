import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../../lib/api";

const APPS_URL = "https://isthereanydeal.com/apps/";

/**
 * El país con el que se piden los precios, sacado del idioma del sistema.
 *
 * Se propone, no se impone: el campo queda editable porque quien tiene el
 * sistema en inglés no compra necesariamente en Estados Unidos, y quien lo
 * tiene en español no compra necesariamente en España.
 */
function paisDelSistema(): string {
  const region = navigator.language.split("-")[1];
  return region === undefined ? "" : region.toUpperCase();
}

/**
 * Precios: ITAD.
 *
 * Una clave más, y por el mismo motivo que las demás: la aplicación no lleva
 * ningún secreto dentro. Aquí la clave es gratis y se saca en un minuto, así
 * que el asistente es corto; lo que no es evidente es el país, y por eso tiene
 * su propio campo en vez de darlo por supuesto.
 */
export function ItadSetup({ onConnected }: { onConnected: () => void }) {
  const [key, setKey] = useState("");
  const [country, setCountry] = useState(paisDelSistema);
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
      <h2>Precios: IsThereAnyDeal</h2>
      <p className="hint">
        Pone precio a tu lista de deseados: cuánto cuesta hoy en cada tienda y
        cuánto ha llegado a costar. Sin esto la lista funciona igual, solo que
        sin precios.
      </p>
      <p className="hint">
        Regístrate en ITAD, entra en tus aplicaciones y crea una: la clave sale
        ahí mismo y es gratis.
      </p>

      <label htmlFor="itad-key">Clave de API de ITAD</label>
      <input
        id="itad-key"
        type="password"
        value={key}
        onChange={(e) => setKey(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        required
      />

      <label htmlFor="itad-country">País</label>
      <input
        id="itad-country"
        value={country}
        onChange={(e) => setCountry(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        maxLength={2}
        required
      />
      {/* Sin país, ITAD contesta con las tiendas y la moneda de otro mercado, y
          el error no se ve: se ven precios, solo que no son los tuyos. */}
      <p className="hint">
        Código de dos letras. Decide en qué tiendas se busca y en qué moneda
        llega el precio.
      </p>

      <button
        type="button"
        className="link"
        onClick={() => {
          openUrl(APPS_URL).catch((cause: unknown) =>
            setError(`No he podido abrir ${APPS_URL}: ${errorMessage(cause)}`),
          );
        }}
      >
        Sacar mi clave en isthereanydeal.com
      </button>

      {error && <p role="alert">{error}</p>}

      <button type="submit" disabled={busy || !key || !country}>
        {busy ? "Comprobando…" : "Guardar"}
      </button>
    </form>
  );
}
