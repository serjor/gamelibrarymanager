import { describe, expect, it } from "bun:test";
import type { LibraryRow } from "../../lib/api";
import { applyFilters, collectGenres, collectStores, EMPTY_FILTERS } from "./filters";

function row(overrides: Partial<LibraryRow>): LibraryRow {
  return {
    game_id: crypto.randomUUID(),
    title: "Juego",
    sort_title: "juego",
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

const biblioteca = [
  row({ title: "Pokémon Rojo", genres: ["RPG"], owned_stores: ["steam"], status: "playing" }),
  row({ title: "Doom", genres: ["Shooter"], owned_stores: ["gog"], status: null }),
  row({ title: "Doom Eternal", genres: ["Shooter"], owned_stores: ["steam", "gog"] }),
];

describe("filtros de biblioteca", () => {
  it("busca sin acentos ni mayúsculas", () => {
    const found = applyFilters(biblioteca, { ...EMPTY_FILTERS, search: "pokemon" });
    expect(found.map((r) => r.title)).toEqual(["Pokémon Rojo"]);
  });

  it("filtra por tienda contando los juegos que están en varias", () => {
    const gog = applyFilters(biblioteca, { ...EMPTY_FILTERS, store: "gog" });
    expect(gog.map((r) => r.title)).toEqual(["Doom", "Doom Eternal"]);
  });

  it("distingue «sin marcar» de «cualquier estado»", () => {
    const sinMarcar = applyFilters(biblioteca, { ...EMPTY_FILTERS, status: "unset" });
    expect(sinMarcar.map((r) => r.title)).toEqual(["Doom", "Doom Eternal"]);
    expect(applyFilters(biblioteca, EMPTY_FILTERS)).toHaveLength(3);
  });

  it("combina búsqueda y género", () => {
    const found = applyFilters(biblioteca, {
      ...EMPTY_FILTERS,
      search: "doom",
      genre: "Shooter",
    });
    expect(found).toHaveLength(2);
  });

  it("recoge tiendas y géneros sin repetir y ordenados", () => {
    expect(collectStores(biblioteca)).toEqual(["gog", "steam"]);
    expect(collectGenres(biblioteca)).toEqual(["RPG", "Shooter"]);
  });

  it("aguanta mil juegos en cada pulsación sin despeinarse", () => {
    const grande = Array.from({ length: 1000 }, (_, i) =>
      row({ title: `Juego ${i}`, genres: [i % 2 ? "RPG" : "Shooter"] }),
    );
    const started = performance.now();
    for (const needle of ["j", "ju", "jue", "juego 9", "juego 99"]) {
      applyFilters(grande, { ...EMPTY_FILTERS, search: needle });
    }
    // Cinco pulsaciones sobre mil filas tienen que costar milisegundos: si esto
    // se dispara, la búsqueda se nota al teclear.
    expect(performance.now() - started).toBeLessThan(100);
  });
});
