import { describe, expect, it } from "bun:test";
import type { LibraryRow } from "../../lib/api";
import { applySort, DEFAULT_SORT } from "./sort";

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

const titulos = (rows: LibraryRow[]) => rows.map((r) => r.title);

describe("ordenación de la biblioteca", () => {
  it("por horas, de más a menos, deja al final lo que no se ha jugado", () => {
    const biblioteca = [
      row({ title: "Sin abrir", sort_title: "sin abrir", playtime_minutes: 0 }),
      row({ title: "Mucho", sort_title: "mucho", playtime_minutes: 3000 }),
      row({ title: "Poco", sort_title: "poco", playtime_minutes: 120 }),
    ];

    expect(titulos(applySort(biblioteca, { field: "hours", desc: true }))).toEqual([
      "Mucho",
      "Poco",
      "Sin abrir",
    ]);
  });

  it("y de menos a más también, porque «sin dato» no es «pocas horas»", () => {
    // Es la regla que más cuesta creerse y la que más se agradece: dar la
    // vuelta al orden para ver lo que menos has jugado no debería llenar la
    // pantalla de lo que no has abierto nunca.
    const biblioteca = [
      row({ title: "Sin abrir", sort_title: "sin abrir", playtime_minutes: 0 }),
      row({ title: "Mucho", sort_title: "mucho", playtime_minutes: 3000 }),
      row({ title: "Poco", sort_title: "poco", playtime_minutes: 120 }),
    ];

    expect(titulos(applySort(biblioteca, { field: "hours", desc: false }))).toEqual([
      "Poco",
      "Mucho",
      "Sin abrir",
    ]);
  });

  it("por última partida deja al final lo que ninguna tienda sabe", () => {
    // Un juego solo de GOG no tiene última partida aunque se haya jugado: la
    // tienda no la publica. No es «hace mucho», es «no hay dato».
    const biblioteca = [
      row({ title: "Solo en GOG", sort_title: "solo en gog", last_played_at: null }),
      row({ title: "Reciente", sort_title: "reciente", last_played_at: 1_750_000_000 }),
      row({ title: "Antiguo", sort_title: "antiguo", last_played_at: 1_400_000_000 }),
    ];

    expect(titulos(applySort(biblioteca, { field: "last", desc: true }))).toEqual([
      "Reciente",
      "Antiguo",
      "Solo en GOG",
    ]);
  });

  it("el título desempata para que el orden no baile entre dos aperturas", () => {
    const biblioteca = [
      row({ title: "Zelda", sort_title: "zelda", release_year: 2017 }),
      row({ title: "Alba", sort_title: "alba", release_year: 2017 }),
    ];

    expect(titulos(applySort(biblioteca, { field: "year", desc: false }))).toEqual([
      "Alba",
      "Zelda",
    ]);
    expect(titulos(applySort(biblioteca, { field: "year", desc: true }))).toEqual([
      "Alba",
      "Zelda",
    ]);
  });

  it("por estado sigue el recorrido de un juego, no el alfabeto", () => {
    const biblioteca = [
      row({ title: "D", sort_title: "d", status: "abandoned" }),
      row({ title: "A", sort_title: "a", status: "backlog" }),
      row({ title: "C", sort_title: "c", status: "finished" }),
      row({ title: "B", sort_title: "b", status: "playing" }),
      row({ title: "E", sort_title: "e", status: null }),
    ];

    expect(titulos(applySort(biblioteca, { field: "status", desc: false }))).toEqual([
      "A",
      "B",
      "C",
      "D",
      "E",
    ]);
  });

  it("ordena sin tocar la lista que recibe", () => {
    const biblioteca = [
      row({ title: "B", sort_title: "b" }),
      row({ title: "A", sort_title: "a" }),
    ];
    applySort(biblioteca, DEFAULT_SORT);
    expect(titulos(biblioteca)).toEqual(["B", "A"]);
  });
});
