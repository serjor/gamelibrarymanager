import { describe, expect, it } from "bun:test";
import type { LibraryRow, PriceRow } from "../../lib/api";
import { deseados, dinero, enMinimoHistorico } from "./precios";

function fila(overrides: Partial<LibraryRow>): LibraryRow {
  const title = overrides.title ?? "Juego";
  return {
    game_id: crypto.randomUUID(),
    title,
    sort_title: title.toLowerCase(),
    cover_url: null,
    summary: null,
    release_year: null,
    genres: [],
    owned_stores: [],
    wishlist_stores: ["steam"],
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

function precio(game_id: string, overrides: Partial<PriceRow> = {}): PriceRow {
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
    itad_slug: "un-juego",
    captured_at: 1_755_000_000,
    ...overrides,
  };
}

describe("lista de deseados", () => {
  it("solo entran los deseados, tengan o no copia en propiedad", () => {
    const enPropiedad = fila({ title: "Hades", wishlist_stores: [], owned_stores: ["steam"] });
    const deseado = fila({ title: "Silksong" });
    // Querer en Steam lo que ya tienes en GOG es una situación real, y el
    // usuario sabrá por qué: no se le esconde de su lista.
    const ambas = fila({ title: "Doom", owned_stores: ["gog"], wishlist_stores: ["steam"] });

    const lista = deseados([enPropiedad, deseado, ambas], []);

    expect(lista.map((d) => d.juego.title).sort()).toEqual(["Doom", "Silksong"]);
  });

  it("ordena por descuento y deja al final lo que no tiene precio", () => {
    const flojo = fila({ title: "Flojo" });
    const rebajado = fila({ title: "Rebajado" });
    const sinPrecio = fila({ title: "Sin precio" });

    const lista = deseados(
      [flojo, rebajado, sinPrecio],
      [precio(flojo.game_id, { cut: 20 }), precio(rebajado.game_id, { cut: 75 })],
    );

    expect(lista.map((d) => d.juego.title)).toEqual(["Rebajado", "Flojo", "Sin precio"]);
    expect(lista[2]?.precio).toBeNull();
  });

  it("con el mismo descuento manda el título, para que el orden no baile", () => {
    const b = fila({ title: "Bastion" });
    const a = fila({ title: "Ape Out" });
    const lista = deseados([b, a], [precio(b.game_id, { cut: 50 }), precio(a.game_id, { cut: 50 })]);
    expect(lista.map((d) => d.juego.title)).toEqual(["Ape Out", "Bastion"]);
  });

  it("distingue un mínimo histórico de un descuento cualquiera", () => {
    // Un −60 % no dice nada si el juego estuvo más barato hace dos meses.
    expect(enMinimoHistorico(precio("x", { amount: 1599, low_all_time: 899 }))).toBe(false);
    expect(enMinimoHistorico(precio("x", { amount: 899, low_all_time: 899 }))).toBe(true);
    // Y un juego que nunca estuvo de oferta no tiene mínimo que igualar.
    expect(enMinimoHistorico(precio("x", { amount: 3999, low_all_time: null }))).toBe(false);
  });

  it("escribe el dinero en su moneda y no en la del sistema", () => {
    expect(dinero(1599, "EUR")).toContain("15,99");
    expect(dinero(0, "EUR")).toContain("0,00");
    expect(dinero(1599, "USD")).toContain("15,99");
    expect(dinero(1599, "USD")).not.toBe(dinero(1599, "EUR"));
  });
});
