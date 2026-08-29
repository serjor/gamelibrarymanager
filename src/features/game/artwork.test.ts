import { describe, expect, it } from "bun:test";
import { artworkVariant, monogram } from "./artwork";

describe("game artwork fallbacks", () => {
  it("creates a two-letter monogram for a one-word title", () => {
    expect(monogram("Hades")).toBe("HA");
    expect(monogram("X")).toBe("X");
  });

  it("uses the first and last words for a multi-word title", () => {
    expect(monogram("The Legend of Zelda")).toBe("TZ");
  });

  it("ignores punctuation and keeps non-ASCII letters", () => {
    expect(monogram("Pokémon: Édition!")).toBe("PÉ");
    expect(monogram("NieR:Automata™")).toBe("NA");
  });

  it("uses a visible marker when normalized text is empty", () => {
    expect(monogram("— … !")).toBe("?");
  });

  it("keeps the same pattern for equivalent title punctuation", () => {
    expect(artworkVariant("The Witcher 3: Wild Hunt")).toBe(
      artworkVariant("The Witcher 3 Wild Hunt"),
    );
  });

  it("returns one of four stable patterns", () => {
    const patterns = new Set(
      ["Alpha", "Bravo", "Charlie", "Delta", "Echo"].map(artworkVariant),
    );

    expect(patterns.size).toBeGreaterThan(0);
    expect([...patterns].every((pattern) => ["a", "b", "c", "d"].includes(pattern))).toBe(true);
    expect(artworkVariant("Same title")).toBe(artworkVariant("Same title"));
  });
});
