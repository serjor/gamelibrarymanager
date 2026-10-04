import { describe, expect, it } from "bun:test";
import type { LunaSettings } from "../../lib/api";
import { LUNA_STALE_SECONDS, lunaCatalogueLabel, lunaIsStale } from "./luna";

function settings(overrides: Partial<LunaSettings> = {}): LunaSettings {
  return {
    country: "ES",
    site: "luna.amazon.es",
    territory: "ES",
    refreshed_at: 1_000_000,
    ...overrides,
  };
}

describe("lunaIsStale", () => {
  it("never asks with Luna off", () => {
    expect(lunaIsStale(null, 9_999_999)).toBe(false);
  });

  it("asks when no refresh succeeded yet", () => {
    expect(lunaIsStale(settings({ refreshed_at: null }), 1)).toBe(true);
  });

  it("asks again after one day and not before", () => {
    const read = settings();
    expect(lunaIsStale(read, 1_000_000 + LUNA_STALE_SECONDS - 1)).toBe(false);
    expect(lunaIsStale(read, 1_000_000 + LUNA_STALE_SECONDS)).toBe(true);
  });
});

describe("lunaCatalogueLabel", () => {
  it("names the territory of the catalogue", () => {
    expect(lunaCatalogueLabel(settings())).toStartWith("Luna catalogue of ES, read on ");
  });

  it("says so when Amazon gave a different country", () => {
    const label = lunaCatalogueLabel(settings({ country: "DE", territory: "ES" }));
    expect(label).toContain("catalogue of ES");
    expect(label).toContain("not of DE");
  });

  it("says when nothing was read yet", () => {
    expect(lunaCatalogueLabel(settings({ refreshed_at: null }))).toBe(
      "Luna catalogue not read yet",
    );
  });
});
