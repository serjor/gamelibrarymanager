import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { errorMessage, type LibraryRow, type PriceRow } from "../../lib/api";
import { deseados, dinero, enMinimoHistorico, type Deseado } from "./precios";

/**
 * Base de las páginas de ITAD. Se escribe como constante y se le concatena el
 * slug, en vez de interpolar la dirección entera, porque el alcance de la
 * capacidad se comprueba contra las cadenas literales que hay en el código.
 */
const ITAD_GAME_URL = "https://isthereanydeal.com/game/";

/**
 * Los deseados, ordenados por descuento.
 *
 * Es la única pantalla que no habla de lo que tienes sino de lo que costaría
 * tenerlo, y por eso no es un tercer modo de vista de la biblioteca: no lee sus
 * filtros y hace su propio corte, como «Hoy».
 *
 * Lo que se enseña de cada juego es lo que hace falta para decidir una compra y
 * nada más: cuánto cuesta hoy, dónde, y si eso es barato de verdad. Un −60 % no
 * dice nada por su cuenta; al lado de su mínimo histórico, sí.
 *
 * El enlace va a la página del juego en ITAD y no a la oferta. No es un rodeo:
 * la oferta apunta a la tienda que sea —Fanatical, Humble, cualquiera—, y la
 * ventana solo puede abrir direcciones que la capacidad enumera de antemano.
 * La página de ITAD las lista todas y es un único host.
 */
export function Wishlist({
  rows,
  precios,
  copias,
  hasItad,
  busy,
  onRefresh,
  onSetup,
}: {
  rows: LibraryRow[];
  precios: PriceRow[];
  /**
   * Cuántas copias deseadas han traído las tiendas.
   *
   * No es lo mismo que la longitud de la lista, y la diferencia es justo lo que
   * hay que explicar: esta pantalla enseña fichas, y una copia sin ficha no
   * aparece por ningún lado. Sin esto, la cabecera decía «84 deseados» y la
   * pantalla enseñaba cero, sin una palabra de por qué.
   */
  copias: number;
  hasItad: boolean;
  busy: boolean;
  onRefresh: () => void;
  onSetup: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const lista = useMemo(() => deseados(rows, precios), [rows, precios]);
  const conPrecio = lista.filter((deseado) => deseado.precio !== null).length;
  // Cuándo se miraron. Un precio de hace una semana ya no es un precio, y sin
  // la fecha no hay forma de saber si lo que se está viendo sigue en pie.
  const capturado = precios.reduce((ultimo, precio) => Math.max(ultimo, precio.captured_at), 0);

  const abrir = (url: string) => {
    openUrl(url).catch((cause: unknown) =>
      setError(`No he podido abrir ${url}: ${errorMessage(cause)}`),
    );
  };

  return (
    <section className="deseados">
      {/* La barra se pinta siempre, también con la lista vacía. Es donde vive
          la única puerta hacia la clave de ITAD, y esconderla hasta que
          hubiera deseados dejaba sin ninguna forma de configurarla a quien
          todavía no había sincronizado. */}
      <div className="deseados-barra">
        <p className="hint">
          {lista.length} deseados
          {conPrecio > 0 && ` · ${conPrecio} con precio`}
          {capturado > 0 && ` · consultados el ${new Date(capturado * 1000).toLocaleString()}`}
        </p>
        {hasItad ? (
          // Con la lista vacía no hay nada que consultar, así que el botón no
          // sale: un botón que no puede hacer nada es una promesa falsa.
          lista.length > 0 && (
            <button disabled={busy} onClick={onRefresh}>
              {busy ? "Consultando precios…" : "Actualizar precios"}
            </button>
          )
        ) : (
          // Sin clave la lista funciona igual, solo que sin precios. Se dice, y
          // no como error: no lo es, igual que no lo es no tener IGDB.
          <p className="hint">
            Sin precios: hace falta una clave de ITAD, que es gratis.{" "}
            <button className="link" onClick={onSetup}>
              Configurar ITAD
            </button>
          </p>
        )}
      </div>

      {error && <p role="alert">{error}</p>}

      {lista.length === 0 &&
        (copias > 0 ? (
          <p className="hint">
            Las tiendas han traído {copias} copias deseadas, pero ninguna tiene
            ficha todavía: esta pantalla enseña fichas, así que sale vacía.
            Pulsa «Emparejar» y déjalo terminar; después vuelve aquí.
          </p>
        ) : (
          <p className="hint">
            No hay ningún juego en tu lista de deseados. Sincroniza una tienda y
            aquí aparecerá lo que te falta por comprar, con su precio.
          </p>
        ))}

      {lista.length > 0 && (
        <div className="deseados-viewport">
          <table className="deseados-tabla">
            <colgroup>
              <col />
              <col style={{ width: "9rem" }} />
              <col style={{ width: "7rem" }} />
              <col style={{ width: "9rem" }} />
              <col style={{ width: "8rem" }} />
              <col style={{ width: "7rem" }} />
            </colgroup>
            <thead>
              <tr>
                <th>Juego</th>
                <th className="num">Mejor precio</th>
                <th className="num">Descuento</th>
                <th className="num">Mínimo histórico</th>
                <th className="num">Mínimo del año</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {lista.map((deseado) => (
                <Fila key={deseado.juego.game_id} deseado={deseado} onAbrir={abrir} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function Fila({ deseado, onAbrir }: { deseado: Deseado; onAbrir: (url: string) => void }) {
  const { juego, precio } = deseado;

  return (
    <tr>
      <td>
        <strong className="deseado-titulo">{juego.title}</strong>
        <span className="hint">
          {juego.wishlist_stores.join(" · ")}
          {/* Lo tienes y lo sigues queriendo: pasa cuando lo quieres en otra
              tienda, y verlo aquí sin explicación parece un fallo. */}
          {juego.owned_stores.length > 0 && ` · ya lo tienes en ${juego.owned_stores.join(", ")}`}
        </span>
      </td>

      {precio === null ? (
        <>
          <td className="num hint">sin precio</td>
          <td className="num hint">—</td>
          <td className="num hint">—</td>
          <td className="num hint">—</td>
          <td />
        </>
      ) : (
        <>
          <td className="num">
            <strong>{dinero(precio.amount, precio.currency)}</strong>
            <span className="hint">
              {precio.shop}
              {precio.shops > 1 && ` · ${precio.shops} tiendas`}
            </span>
          </td>
          <td className="num">
            {precio.cut > 0 ? (
              <>
                <span className="descuento">−{precio.cut}%</span>
                <span className="hint">{dinero(precio.regular, precio.currency)}</span>
              </>
            ) : (
              <span className="hint">sin rebaja</span>
            )}
          </td>
          <td className="num">
            {precio.low_all_time === null ? (
              <span className="hint">nunca rebajado</span>
            ) : (
              <>
                {dinero(precio.low_all_time, precio.currency)}
                {enMinimoHistorico(precio) && <span className="minimo">en su mínimo</span>}
              </>
            )}
          </td>
          <td className="num">
            {precio.low_year === null ? (
              <span className="hint">—</span>
            ) : (
              dinero(precio.low_year, precio.currency)
            )}
          </td>
          <td className="num">
            {precio.itad_slug !== null && (
              <button
                className="link"
                onClick={() => onAbrir(`${ITAD_GAME_URL}${precio.itad_slug}/info/`)}
              >
                Ver precios ↗
              </button>
            )}
          </td>
        </>
      )}
    </tr>
  );
}
