import type { LibraryRow, PriceRow } from "../../lib/api";

/**
 * The rules of the wishlist, pure and out of React, as the filter and the sort
 * of the library are.
 *
 * What turns a wishlist into a decision to buy is the order: the largest
 * discount first. A wishlist sorted by title is a list of good intentions.
 */

export interface Wish {
  game: LibraryRow;
  /** Absent while nobody has asked for the prices, or if nobody sells it. */
  price: PriceRow | null;
}

/**
 * The wished-for games with their price beside them, the largest discount first.
 *
 * The games with no price go last, not first and not mixed in: that is "no
 * data", as with the hours that no store publishes, and they are two different
 * questions. Between equal games the title decides, so that the order stays.
 */
export function wishes(rows: LibraryRow[], prices: PriceRow[]): Wish[] {
  const byGame = new Map(prices.map((price) => [price.game_id, price]));

  return rows
    .filter((row) => row.wishlist_stores.length > 0)
    .map((game) => ({ game, price: byGame.get(game.game_id) ?? null }))
    .sort(byDiscount);
}

function byDiscount(a: Wish, b: Wish): number {
  if (a.price === null && b.price === null) return byTitle(a, b);
  if (a.price === null) return 1;
  if (b.price === null) return -1;
  return b.price.cut - a.price.cut || byTitle(a, b);
}

function byTitle(a: Wish, b: Wish): number {
  return a.game.sort_title.localeCompare(b.game.sort_title, "en");
}

/**
 * Whether the price now is equal to or better than the all-time low.
 *
 * It is the only question that a person who looks at this screen really asks: a
 * −60 % tells you nothing if the game was at −75 % two months ago. The
 * comparison uses `<=` because the ITAD low includes the offer in progress:
 * without that, the best discount in the history of a game would never be
 * marked.
 */
export function atAllTimeLow(price: PriceRow): boolean {
  return price.low_all_time !== null && price.amount <= price.low_all_time;
}

/**
 * Cents to text, in the currency that the price itself gives.
 *
 * The language is set to English, as with the tie break by title of the library:
 * it is the language of the application, and to let the system decide would make
 * the same quantity appear in two forms on two machines.
 */
export function money(cents: number, currency: string): string {
  return new Intl.NumberFormat("en-GB", { style: "currency", currency }).format(cents / 100);
}
