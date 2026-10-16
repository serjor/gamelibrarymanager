import { describe, expect, it } from "bun:test";
import type { SteamFamilySettings } from "../../lib/api";
import { familyLabel } from "./family";

function settings(overrides: Partial<SteamFamilySettings> = {}): SteamFamilySettings {
  return {
    family_groupid: "1234567890123456789",
    refreshed_at: 1_000_000,
    last_error: null,
    games: 42,
    ...overrides,
  };
}

describe("familyLabel", () => {
  it("says how many games the family can lend and when", () => {
    expect(familyLabel(settings())).toStartWith("42 games that your family can lend, read on ");
    expect(familyLabel(settings({ games: 1 }))).toStartWith("1 game that ");
  });

  it("says when Steam knows no family, and that is not an error", () => {
    expect(familyLabel(settings({ family_groupid: null, games: 0 }))).toStartWith(
      "Steam says that you are in no family",
    );
  });

  it("keeps the date of the marks and says why the last read failed", () => {
    const label = familyLabel(settings({ last_error: "the Steam sign in was closed" }));
    expect(label).toContain("read on ");
    expect(label).toEndWith("The last synchronisation did not read it: the Steam sign in was closed");
  });

  it("before the first read it says so", () => {
    expect(familyLabel(settings({ refreshed_at: null }))).toBe("Steam Family not read yet");
  });
});
