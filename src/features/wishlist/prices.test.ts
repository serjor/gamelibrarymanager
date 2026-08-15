import { describe, expect, it } from "bun:test";
import type { LibraryRow, PriceRow } from "../../lib/api";
import { wishes, money, atAllTimeLow } from "./prices";

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

describe("the wishlist", () => {
  it("only the wished-for games come in, with or without an owned copy", () => {
    const owned = row({ title: "Hades", wishlist_stores: [], owned_stores: ["steam"] });
    const wished = row({ title: "Silksong" });
    // To want in Steam what you already have in GOG is a real condition, and the
    // user will know why: the list does not hide it from them.
    const both = row({ title: "Doom", owned_stores: ["gog"], wishlist_stores: ["steam"] });

    const list = wishes([owned, wished, both], []);

    expect(list.map((d) => d.game.title).sort()).toEqual(["Doom", "Silksong"]);
  });

  it("sorts by discount and puts last the games with no price", () => {
    const small = row({ title: "Small" });
    const large = row({ title: "Large" });
    const noPrice = row({ title: "No price" });

    const list = wishes(
      [small, large, noPrice],
      [price(small.game_id, { cut: 20 }), price(large.game_id, { cut: 75 })],
    );

    expect(list.map((d) => d.game.title)).toEqual(["Large", "Small", "No price"]);
    expect(list[2]?.price).toBeNull();
  });

  it("with the same discount the title decides, so that the order stays", () => {
    const b = row({ title: "Bastion" });
    const a = row({ title: "Ape Out" });
    const list = wishes([b, a], [price(b.game_id, { cut: 50 }), price(a.game_id, { cut: 50 })]);
    expect(list.map((d) => d.game.title)).toEqual(["Ape Out", "Bastion"]);
  });

  it("tells an all-time low from any other discount", () => {
    // A −60 % tells you nothing if the game was less expensive two months ago.
    expect(atAllTimeLow(price("x", { amount: 1599, low_all_time: 899 }))).toBe(false);
    expect(atAllTimeLow(price("x", { amount: 899, low_all_time: 899 }))).toBe(true);
    // And a game that has never been on offer has no low to be equal to.
    expect(atAllTimeLow(price("x", { amount: 3999, low_all_time: null }))).toBe(false);
  });

  it("writes the money in its own currency and not in the currency of the system", () => {
    expect(money(1599, "EUR")).toContain("15.99");
    expect(money(0, "EUR")).toContain("0.00");
    expect(money(1599, "USD")).toContain("15.99");
    expect(money(1599, "USD")).not.toBe(money(1599, "EUR"));
  });
});
