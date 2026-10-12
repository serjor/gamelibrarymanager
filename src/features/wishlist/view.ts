import type { PlatformFamily } from "../../lib/api";
import { fold } from "../library/filters";
import { atAllTimeLow, type Wish } from "./prices";

/**
 * The filter and the sort of the wishlist, pure and out of React, as the ones of
 * the library are.
 *
 * They are not the filters of the library: this screen makes its own division
 * (see `Wishlist.tsx`), and the questions are different. Here you do not ask
 * about a status or about hours, you ask what is inexpensive now, where you want
 * it, and whether Luna already gives it to you.
 */

export interface WishFilters {
  search: string;
  /** A store whose wishlist has the game. */
  store: string | null;
  /** A device family of a wish that the user added by hand. */
  device: PlatformFamily | null;
  price: "priced" | "none" | "discounted" | "low" | null;
  luna: "on" | "off" | null;
}

export const EMPTY_WISH_FILTERS: WishFilters = {
  search: "",
  store: null,
  device: null,
  price: null,
  luna: null,
};

export function applyWishFilters(list: Wish[], filters: WishFilters): Wish[] {
  const needle = fold(filters.search.trim());

  return list.filter(({ game, price }) => {
    if (needle && !fold(game.title).includes(needle)) return false;
    if (filters.store && !game.wishlist_stores.includes(filters.store)) return false;
    if (filters.device && !game.manual_wishes.some((wish) => wish.family === filters.device)) return false;
    if (filters.price === "priced" && price === null) return false;
    if (filters.price === "none" && price !== null) return false;
    if (filters.price === "discounted" && (price === null || price.cut <= 0)) return false;
    if (filters.price === "low" && (price === null || !atAllTimeLow(price))) return false;
    if (filters.luna === "on" && game.luna === undefined) return false;
    if (filters.luna === "off" && game.luna !== undefined) return false;
    return true;
  });
}

export function collectWishStores(list: Wish[]): string[] {
  return [...new Set(list.flatMap(({ game }) => game.wishlist_stores))].sort();
}

export function collectWishDevices(list: Wish[]): PlatformFamily[] {
  return [...new Set(list.flatMap(({ game }) => game.manual_wishes.map((wish) => wish.family)))];
}

export type WishSortField = "title" | "price" | "discount" | "low" | "year";

export interface WishSort {
  field: WishSortField;
  desc: boolean;
}

/** The largest discount first: what turns a wishlist into a decision to buy. */
export const DEFAULT_WISH_SORT: WishSort = { field: "discount", desc: true };

/**
 * The direction that a first click on a column gives. A discount is read from
 * the largest; a price, from the lowest.
 */
export function firstDirection(field: WishSortField): boolean {
  return field === "discount";
}

function value({ game, price }: Wish, field: WishSortField): string | number | null {
  switch (field) {
    case "title":
      return game.sort_title;
    case "price":
      return price?.amount ?? null;
    case "discount":
      return price?.cut ?? null;
    case "low":
      return price?.low_all_time ?? null;
    case "year":
      return price?.low_year ?? null;
  }
}

function byTitle(a: Wish, b: Wish): number {
  return a.game.sort_title.localeCompare(b.game.sort_title, "en");
}

/**
 * The games with no data go last **in the two directions**, as in the library:
 * to invert the order to see the most expensive must not fill the first screen
 * with the games that have no price. Between equal games the title decides, so
 * that the order stays.
 */
export function applyWishSort(list: Wish[], sort: WishSort): Wish[] {
  const direction = sort.desc ? -1 : 1;

  return [...list].sort((a, b) => {
    const va = value(a, sort.field);
    const vb = value(b, sort.field);

    if (va === null && vb === null) return byTitle(a, b);
    if (va === null) return 1;
    if (vb === null) return -1;

    const compared =
      typeof va === "string" && typeof vb === "string"
        ? va.localeCompare(vb, "en")
        : Number(va) - Number(vb);

    return compared === 0 ? byTitle(a, b) : compared * direction;
  });
}
