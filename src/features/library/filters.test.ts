import { describe, expect, it } from "bun:test";
import type { LibraryRow } from "../../lib/api";
import { applyFilters, collectGenres, collectStores, EMPTY_FILTERS } from "./filters";

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

const library = [
  row({ title: "Pokémon Red", genres: ["RPG"], owned_stores: ["steam"], status: "playing" }),
  row({ title: "Doom", genres: ["Shooter"], owned_stores: ["gog"], status: null }),
  row({ title: "Doom Eternal", genres: ["Shooter"], owned_stores: ["steam", "gog"] }),
];

describe("the library filters", () => {
  it("searches with no accents and no capitals", () => {
    const found = applyFilters(library, { ...EMPTY_FILTERS, search: "pokemon" });
    expect(found.map((r) => r.title)).toEqual(["Pokémon Red"]);
  });

  it("filters by store and counts the games that are in more than one", () => {
    const gog = applyFilters(library, { ...EMPTY_FILTERS, store: "gog" });
    expect(gog.map((r) => r.title)).toEqual(["Doom", "Doom Eternal"]);
  });

  it("tells \"not marked\" from \"any status\"", () => {
    const notMarked = applyFilters(library, { ...EMPTY_FILTERS, status: "unset" });
    expect(notMarked.map((r) => r.title)).toEqual(["Doom", "Doom Eternal"]);
    expect(applyFilters(library, EMPTY_FILTERS)).toHaveLength(3);
  });

  it("combines a search and a genre", () => {
    const found = applyFilters(library, {
      ...EMPTY_FILTERS,
      search: "doom",
      genre: "Shooter",
    });
    expect(found).toHaveLength(2);
  });

  it("collects the stores and the genres with no repeats and sorted", () => {
    expect(collectStores(library)).toEqual(["gog", "steam"]);
    expect(collectGenres(library)).toEqual(["RPG", "Shooter"]);
  });

  it("holds one thousand games at each key press with no difficulty", () => {
    const large = Array.from({ length: 1000 }, (_, i) =>
      row({ title: `Game ${i}`, genres: [i % 2 ? "RPG" : "Shooter"] }),
    );
    const started = performance.now();
    for (const needle of ["g", "ga", "gam", "game 9", "game 99"]) {
      applyFilters(large, { ...EMPTY_FILTERS, search: needle });
    }
    // Five key presses over one thousand rows must cost milliseconds: if this
    // grows, the user feels the search while they type.
    expect(performance.now() - started).toBeLessThan(100);
  });
});
