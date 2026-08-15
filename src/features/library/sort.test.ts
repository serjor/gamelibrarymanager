import { describe, expect, it } from "bun:test";
import type { LibraryRow } from "../../lib/api";
import { applySort, DEFAULT_SORT } from "./sort";

function row(overrides: Partial<LibraryRow>): LibraryRow {
  return {
    game_id: crypto.randomUUID(),
    title: "Game",
    sort_title: "game",
    cover_url: null,
    summary: null,
    release_year: null,
    genres: [],
    owned_stores: ["steam"],
    wishlist_stores: [],
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

const titles = (rows: LibraryRow[]) => rows.map((r) => r.title);

describe("the sort of the library", () => {
  it("by hours, from most to least, puts the games not played last", () => {
    const library = [
      row({ title: "Never opened", sort_title: "never opened", playtime_minutes: 0 }),
      row({ title: "Many", sort_title: "many", playtime_minutes: 3000 }),
      row({ title: "Few", sort_title: "few", playtime_minutes: 120 }),
    ];

    expect(titles(applySort(library, { field: "hours", desc: true }))).toEqual([
      "Many",
      "Few",
      "Never opened",
    ]);
  });

  it('and from least to most too, because "no data" is not "few hours"', () => {
    // It is the rule that is hardest to believe and the most useful: to invert
    // the order to see the games that you have played least must not fill the
    // screen with the games that you have never opened.
    const library = [
      row({ title: "Never opened", sort_title: "never opened", playtime_minutes: 0 }),
      row({ title: "Many", sort_title: "many", playtime_minutes: 3000 }),
      row({ title: "Few", sort_title: "few", playtime_minutes: 120 }),
    ];

    expect(titles(applySort(library, { field: "hours", desc: false }))).toEqual([
      "Few",
      "Many",
      "Never opened",
    ]);
  });

  it("by the last game played puts last what no store knows", () => {
    // A game that is only in GOG has no last game played even if you played it:
    // the store does not publish it. It is not "a long time ago", it is "no
    // data".
    const library = [
      row({ title: "Only in GOG", sort_title: "only in gog", last_played_at: null }),
      row({ title: "Recent", sort_title: "recent", last_played_at: 1_750_000_000 }),
      row({ title: "Old", sort_title: "old", last_played_at: 1_400_000_000 }),
    ];

    expect(titles(applySort(library, { field: "last", desc: true }))).toEqual([
      "Recent",
      "Old",
      "Only in GOG",
    ]);
  });

  it("the title breaks the tie so that the order stays between two starts", () => {
    const library = [
      row({ title: "Zelda", sort_title: "zelda", release_year: 2017 }),
      row({ title: "Alba", sort_title: "alba", release_year: 2017 }),
    ];

    expect(titles(applySort(library, { field: "year", desc: false }))).toEqual([
      "Alba",
      "Zelda",
    ]);
    expect(titles(applySort(library, { field: "year", desc: true }))).toEqual([
      "Alba",
      "Zelda",
    ]);
  });

  it("by status it follows the path of a game, not the alphabet", () => {
    const library = [
      row({ title: "D", sort_title: "d", status: "abandoned" }),
      row({ title: "A", sort_title: "a", status: "backlog" }),
      row({ title: "C", sort_title: "c", status: "finished" }),
      row({ title: "B", sort_title: "b", status: "playing" }),
      row({ title: "E", sort_title: "e", status: null }),
    ];

    expect(titles(applySort(library, { field: "status", desc: false }))).toEqual([
      "A",
      "B",
      "C",
      "D",
      "E",
    ]);
  });

  it("sorts and does not touch the list that it receives", () => {
    const library = [
      row({ title: "B", sort_title: "b" }),
      row({ title: "A", sort_title: "a" }),
    ];
    applySort(library, DEFAULT_SORT);
    expect(titles(library)).toEqual(["B", "A"]);
  });
});
