import { describe, expect, it } from "bun:test";
import type { LibraryRow, PriceRow } from "../../lib/api";
import { wishes } from "./prices";
import {
  DEFAULT_WISH_SORT,
  EMPTY_WISH_FILTERS,
  applyWishFilters,
  applyWishSort,
  collectWishDevices,
  collectWishStores,
} from "./view";

function row(overrides: Partial<LibraryRow>): LibraryRow {
  const title = overrides.title ?? "Game";
  return {
    game_id: crypto.randomUUID(),
    title,
    sort_title: title.toLowerCase(),
    cover_url: null,
    summary: null,
    release_year: null,
    genres: [],
    time_to_beat: null,
    owned_stores: [],
    wishlist_stores: ["steam"],
    manual_wishes: [],
    store_cover_url: null,
    store_url: null,
    playtime_minutes: 0,
    last_played_at: null,
    status: null,
    rating: null,
    notes: null,
    ...overrides,
  };
}

function price(game_id: string, overrides: Partial<PriceRow> = {}): PriceRow {
  return {
    game_id,
    shop: "GOG",
    amount: 1599,
    regular: 3999,
    cut: 60,
    currency: "EUR",
    shops: 3,
    low_all_time: 899,
    low_year: 1349,
    itad_slug: "a-game",
    captured_at: 1_755_000_000,
    ...overrides,
  };
}

const hades = row({ title: "Hades", wishlist_stores: ["gog"] });
const pokemon = row({
  title: "Pokémon Legends",
  wishlist_stores: [],
  manual_wishes: [{ id: "w1", game_id: "x", family: "nintendo", model: "Switch 2" }],
});
const silksong = row({ title: "Silksong", luna: { title: "Silksong", url: "https://luna.amazon.com/x" } });
const tunic = row({ title: "Tunic" });

const LIST = wishes(
  [hades, pokemon, silksong, tunic],
  [
    price(hades.game_id, { amount: 2399, cut: 40, low_all_time: 1999, low_year: 2399 }),
    price(silksong.game_id, { amount: 899, cut: 75, low_all_time: 899, low_year: null }),
    price(tunic.game_id, { amount: 2499, cut: 0, regular: 2499, low_all_time: null, low_year: null }),
  ],
);

const titles = (list: typeof LIST) => list.map((wish) => wish.game.title);

describe("the wishlist filters", () => {
  it("with no filter nothing goes away", () => {
    expect(applyWishFilters(LIST, EMPTY_WISH_FILTERS)).toHaveLength(4);
  });

  it("the search ignores accents and capitals", () => {
    const found = applyWishFilters(LIST, { ...EMPTY_WISH_FILTERS, search: "POKEMON" });
    expect(titles(found)).toEqual(["Pokémon Legends"]);
  });

  it("filters by the store of the wishlist and by the device of a manual wish", () => {
    expect(titles(applyWishFilters(LIST, { ...EMPTY_WISH_FILTERS, store: "gog" }))).toEqual(["Hades"]);
    expect(titles(applyWishFilters(LIST, { ...EMPTY_WISH_FILTERS, device: "nintendo" }))).toEqual([
      "Pokémon Legends",
    ]);
  });

  it("filters by what the price says", () => {
    const by = (value: NonNullable<typeof EMPTY_WISH_FILTERS.price>) =>
      titles(applyWishFilters(LIST, { ...EMPTY_WISH_FILTERS, price: value })).sort();

    expect(by("priced")).toEqual(["Hades", "Silksong", "Tunic"]);
    expect(by("none")).toEqual(["Pokémon Legends"]);
    // No discount is not a discount.
    expect(by("discounted")).toEqual(["Hades", "Silksong"]);
    expect(by("low")).toEqual(["Silksong"]);
  });

  it("filters by Luna", () => {
    expect(titles(applyWishFilters(LIST, { ...EMPTY_WISH_FILTERS, luna: "on" }))).toEqual(["Silksong"]);
    expect(applyWishFilters(LIST, { ...EMPTY_WISH_FILTERS, luna: "off" })).toHaveLength(3);
  });

  it("offers only the stores and the devices that the list has", () => {
    expect(collectWishStores(LIST)).toEqual(["gog", "steam"]);
    expect(collectWishDevices(LIST)).toEqual(["nintendo"]);
  });
});

describe("the wishlist sort", () => {
  it("by default, the largest discount first and no price last", () => {
    expect(titles(applyWishSort(LIST, DEFAULT_WISH_SORT))).toEqual([
      "Silksong",
      "Hades",
      "Tunic",
      "Pokémon Legends",
    ]);
  });

  it("by price in the two directions, and no price is last in both", () => {
    expect(titles(applyWishSort(LIST, { field: "price", desc: false }))).toEqual([
      "Silksong",
      "Hades",
      "Tunic",
      "Pokémon Legends",
    ]);
    expect(titles(applyWishSort(LIST, { field: "price", desc: true }))).toEqual([
      "Tunic",
      "Hades",
      "Silksong",
      "Pokémon Legends",
    ]);
  });

  it("by title", () => {
    expect(titles(applyWishSort(LIST, { field: "title", desc: false }))).toEqual([
      "Hades",
      "Pokémon Legends",
      "Silksong",
      "Tunic",
    ]);
  });

  it("by the lows, with a game that was never discounted last", () => {
    expect(titles(applyWishSort(LIST, { field: "low", desc: false }))).toEqual([
      "Silksong",
      "Hades",
      "Pokémon Legends",
      "Tunic",
    ]);
    expect(titles(applyWishSort(LIST, { field: "year", desc: true }))).toEqual([
      "Hades",
      "Pokémon Legends",
      "Silksong",
      "Tunic",
    ]);
  });
});
