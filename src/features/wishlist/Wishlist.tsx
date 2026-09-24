import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { errorMessage, type LibraryRow, type PlatformFamily, type PriceRow, type WishTarget } from "../../lib/api";
import { wishes, money, atAllTimeLow, type Wish } from "./prices";
import { ManualWishForm, FAMILIES, deviceLabel } from "./ManualWishForm";

/**
 * The base of the ITAD pages. It is written as a constant and the slug is added
 * to it, and the complete address is not interpolated, because the scope of the
 * capability is examined against the literal strings that are in the code.
 */
const ITAD_GAME_URL = "https://isthereanydeal.com/game/";

/**
 * The wished-for games, sorted by discount.
 *
 * It is the only screen that speaks not about what you have but about what it
 * would cost to have it, and thus it is not a third view mode of the library: it
 * does not read the library filters and it makes its own division, as "Today"
 * does.
 *
 * What it shows about each game is what is necessary to decide a purchase and
 * nothing else: what it costs today, where, and whether that is really
 * inexpensive. A −60 % tells you nothing alone; beside its all-time low, it
 * does.
 *
 * The link goes to the page of the game in ITAD and not to the offer. That is
 * not an unnecessary step: the offer points to a store that can be any store —
 * Fanatical, Humble, any of them — and the window can open only the addresses
 * that the capability lists before. The ITAD page lists all of them and it is
 * one host.
 */
export function Wishlist({
  rows,
  prices,
  copies,
  hasItad,
  hasIgdb,
  busy,
  onRefresh,
  onSetup,
  onAdd,
  onUpdate,
  onRemove,
}: {
  rows: LibraryRow[];
  prices: PriceRow[];
  /**
   * How many wished-for copies the stores gave.
   *
   * It is not the same as the length of the list, and the difference is exactly
   * what must be explained: this screen shows records, and a copy with no record
   * appears in no place. Without this, the header said "84 wished for" and the
   * screen showed zero, with no word about why.
   */
  copies: number;
  hasItad: boolean;
  hasIgdb: boolean;
  busy: boolean;
  onRefresh: () => void;
  onSetup: () => void;
  onAdd: (target: WishTarget, family: PlatformFamily, model: string) => Promise<void>;
  onUpdate: (wishId: string, family: PlatformFamily, model: string) => Promise<void>;
  onRemove: (wishId: string) => Promise<void>;
}) {
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [editFamily, setEditFamily] = useState<PlatformFamily>("pc");
  const [editModel, setEditModel] = useState("");
  const [saving, setSaving] = useState(false);
  const list = useMemo(() => wishes(rows, prices), [rows, prices]);
  const withPrice = list.filter((wish) => wish.price !== null).length;
  const hasPriceTargets = list.some(({ game }) =>
    game.wishlist_stores.length > 0 || game.manual_wishes.some((wish) => wish.family === "pc"),
  );
  // When the prices were read. A price from one week ago is no longer a price,
  // and with no date there is no way to know whether what you see still applies.
  const captured = prices.reduce((last, price) => Math.max(last, price.captured_at), 0);

  const open = (url: string) => {
    openUrl(url).catch((cause: unknown) =>
      setError(`Could not open ${url}: ${errorMessage(cause)}`),
    );
  };

  const change = async (action: () => Promise<void>) => {
    setSaving(true);
    setError(null);
    try {
      await action();
      setEditing(null);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <section className="wishlist command-deck">
      {/* The bar is always shown, also with an empty list. It is where the only
          door to the ITAD key lives, and to hide it until there were wished-for
          games left a user who had not yet synchronised with no way to configure
          it. */}
      <div className="wishlist-bar command-toolbar">
        <div className="wishlist-summary">
          <p className="hint" role="status">
            {list.length} wished for
            {withPrice > 0 && ` · ${withPrice} with a price`}
            {captured > 0 && ` · read on ${new Date(captured * 1000).toLocaleString()}`}
          </p>
        </div>
        <div className="wishlist-actions">
          <button type="button" disabled={busy || saving} onClick={() => setAdding((value) => !value)}>
            {adding ? "Close form" : "Add wanted game"}
          </button>
          {hasItad ? (
            // With an empty list there is nothing to ask about, thus the button
            // does not appear: a button that can do nothing is a false promise.
            hasPriceTargets && (
              <button disabled={busy} onClick={onRefresh}>
                {busy ? "Reading prices…" : "Update the prices"}
              </button>
            )
          ) : (
            // With no key the list operates in the same way, only with no prices.
            // The interface says that, and not as an error: it is not an error, in
            // the same way as no IGDB is not an error.
            <p className="hint">
              No prices: an ITAD key is necessary, and it is free.{" "}
              <button className="link" onClick={onSetup}>
                Configure ITAD
              </button>
            </p>
          )}
        </div>
      </div>

      {error && <p role="alert">{error}</p>}

      {adding && <ManualWishForm rows={rows} hasIgdb={hasIgdb} onAdd={onAdd} onClose={() => setAdding(false)} />}

      {list.length === 0 && (
        <div className="empty-state" role="status">
          <strong className="empty-state-title">
            {copies > 0 ? "Records need matching" : "Wishlist is empty"}
          </strong>
          {copies > 0 ? (
          <p className="hint">
            The stores gave {copies} wished-for copies, but none of them has a
            record yet: this screen shows records, thus it is empty. Click
            "Match" and let it finish; then come back here.
          </p>
          ) : (
          <p className="hint">
            There is no game in your wishlist. Add a wanted game for a device,
            or synchronise a store.
          </p>
          )}
        </div>
      )}

      {list.length > 0 && (
        <div className="wishlist-viewport">
          <table className="wishlist-table command-table" aria-label="Wishlist prices">
            <colgroup>
              <col />
              <col style={{ width: "9rem" }} />
              <col style={{ width: "7rem" }} />
              <col style={{ width: "9rem" }} />
              <col style={{ width: "8rem" }} />
              <col style={{ width: "8rem" }} />
            </colgroup>
            <thead>
              <tr>
                <th>Game</th>
                <th className="num">Best price</th>
                <th className="num">Discount</th>
                <th className="num">All-time low</th>
                <th className="num">Low of the year</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {list.map((wish) => (
                <Row key={wish.game.game_id} wish={wish} onOpen={open}
                  editing={editing} editFamily={editFamily} editModel={editModel} saving={saving}
                  onStartEdit={(id, family, model) => { setEditing(id); setEditFamily(family); setEditModel(model); setError(null); }}
                  onFamily={setEditFamily} onModel={setEditModel} onCancelEdit={() => setEditing(null)}
                  onSaveEdit={(id) => void change(() => onUpdate(id, editFamily, editModel))}
                  onRemove={(id) => void change(() => onRemove(id))} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function Row({ wish, onOpen, editing, editFamily, editModel, saving, onStartEdit, onFamily, onModel, onCancelEdit, onSaveEdit, onRemove }: {
  wish: Wish;
  onOpen: (url: string) => void;
  editing: string | null;
  editFamily: PlatformFamily;
  editModel: string;
  saving: boolean;
  onStartEdit: (id: string, family: PlatformFamily, model: string) => void;
  onFamily: (family: PlatformFamily) => void;
  onModel: (model: string) => void;
  onCancelEdit: () => void;
  onSaveEdit: (id: string) => void;
  onRemove: (id: string) => void;
}) {
  const { game, price } = wish;

  return (
    <tr>
      <td>
        <strong className="wish-title">{game.title}</strong>
        <span className="hint">
          {game.wishlist_stores.length > 0 && `Store wishlist: ${game.wishlist_stores.join(" · ")}`}
          {/* You have it and you still want it: that occurs when you want it in
              a different store, and to see it here with no explanation looks
              like a defect. */}
          {game.owned_stores.length > 0 && ` · you already have it in ${game.owned_stores.join(", ")}`}
        </span>
        {game.manual_wishes.length > 0 && <div className="manual-wish-list" aria-label={`Devices wanted for ${game.title}`}>
          {game.manual_wishes.map((manual) => <div className="manual-wish-device" key={manual.id}>
            {editing === manual.id ? <>
              <select aria-label={`Device family for ${game.title}`} value={editFamily} onChange={(event) => onFamily(event.target.value as PlatformFamily)}>
                {FAMILIES.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
              </select>
              <input aria-label={`Device model for ${game.title}`} value={editModel} onChange={(event) => onModel(event.target.value)} maxLength={80} />
              <button type="button" disabled={saving || (editFamily === "other" && !editModel.trim())} onClick={() => onSaveEdit(manual.id)}>Save</button>
              <button type="button" className="link" onClick={onCancelEdit}>Cancel</button>
            </> : <>
              <span>{deviceLabel(manual.family, manual.model)}</span>
              <button type="button" className="link" disabled={saving} aria-label={`Edit ${deviceLabel(manual.family, manual.model)} wish for ${game.title}`} onClick={() => onStartEdit(manual.id, manual.family, manual.model)}>Edit</button>
              <button type="button" className="link" disabled={saving} aria-label={`Remove ${deviceLabel(manual.family, manual.model)} wish for ${game.title}`} onClick={() => onRemove(manual.id)}>Remove</button>
            </>}
          </div>)}
        </div>}
      </td>

      {price === null ? (
        <>
          <td className="num hint">no price</td>
          <td className="num hint">—</td>
          <td className="num hint">—</td>
          <td className="num hint">—</td>
          <td />
        </>
      ) : (
        <>
          <td className="num">
            <strong>{money(price.amount, price.currency)}</strong>
            <span className="hint">
              {price.shop}
              {price.shops > 1 && ` · ${price.shops} stores`}
            </span>
          </td>
          <td className="num">
            {price.cut > 0 ? (
              <>
                <span className="discount">−{price.cut}%</span>
                <span className="hint">{money(price.regular, price.currency)}</span>
              </>
            ) : (
              <span className="hint">no discount</span>
            )}
          </td>
          <td className="num">
            {price.low_all_time === null ? (
              <span className="hint">never discounted</span>
            ) : (
              <>
                {money(price.low_all_time, price.currency)}
                {atAllTimeLow(price) && <span className="low">at its low</span>}
              </>
            )}
          </td>
          <td className="num">
            {price.low_year === null ? (
              <span className="hint">—</span>
            ) : (
              money(price.low_year, price.currency)
            )}
          </td>
          <td className="num">
            {price.itad_slug !== null && (
              <button
                className="link"
                onClick={() => onOpen(`${ITAD_GAME_URL}${price.itad_slug}/info/`)}
              >
                See the prices ↗
              </button>
            )}
          </td>
        </>
      )}
    </tr>
  );
}
