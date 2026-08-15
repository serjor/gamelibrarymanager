import { describe, expect, it } from "bun:test";
import type { LibraryRow } from "../../lib/api";
import { featured, shelves } from "./shelves";

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

/** A fixed moment: the rules divide at "six months ago". */
const NOW = 1_760_000_000;
const DAY = 86_400;

const titles = (rows: LibraryRow[]) => rows.map((r) => r.title);
const ids = (rows: LibraryRow[]) => shelves(rows, NOW).map((e) => e.id);

describe("the shelves of Today", () => {
  it("an empty library gives back no shelf", () => {
    // That is not the same as empty shelves: the screen shows what there is, and
    // a list of headings with nothing below them tells you nothing.
    expect(shelves([], NOW)).toEqual([]);
    expect(featured([], NOW)).toBeNull();
  });

  it("a wished-for game is not proposed: you cannot play it today", () => {
    const wished = [row({ title: "Wished", owned_stores: [], wishlist_stores: ["steam"] })];
    expect(shelves(wished, NOW)).toEqual([]);
    expect(featured(wished, NOW)).toBeNull();
  });

  it("it gives back only the shelves that have something in them", () => {
    // Nothing started and nothing in two stores: those two do not appear.
    const library = [
      row({ title: "Never opened" }),
      row({ title: "Left", playtime_minutes: 600, last_played_at: NOW - 400 * DAY }),
    ];
    expect(ids(library)).toEqual(["not-touched", "never-started"]);
  });

  it('"you have not touched it for a long time" starts with the oldest', () => {
    const library = [
      row({ title: "One year ago", playtime_minutes: 600, last_played_at: NOW - 370 * DAY }),
      row({ title: "Three years ago", playtime_minutes: 600, last_played_at: NOW - 1100 * DAY }),
      // Exactly below the limit: it does not come in.
      row({ title: "One month ago", playtime_minutes: 600, last_played_at: NOW - 30 * DAY }),
    ];
    const shelf = shelves(library, NOW).find((e) => e.id === "not-touched");
    expect(titles(shelf?.games ?? [])).toEqual(["Three years ago", "One year ago"]);
  });

  it("the finished and abandoned games are not proposed again", () => {
    // It is a decision already made, and "Today" does not open it again. It does
    // still count for "you have it two times", which is not a proposal but data
    // about the copy.
    const library = [
      row({
        title: "Finished",
        status: "finished",
        playtime_minutes: 600,
        last_played_at: NOW - 400 * DAY,
      }),
      row({ title: "Abandoned", status: "abandoned", owned_stores: ["steam", "gog"] }),
    ];
    expect(ids(library)).toEqual(["two-times"]);
  });

  it('a game only in GOG does not count as "never started" if you played it', () => {
    // GOG does not publish the last game played, thus it comes with no date.
    // What you can declare is the hours: with hours played, it is started.
    const library = [
      row({ title: "From GOG played", owned_stores: ["gog"], playtime_minutes: 240 }),
      row({ title: "From GOG never opened", owned_stores: ["gog"] }),
    ];
    const shelf = shelves(library, NOW).find((e) => e.id === "never-started");
    expect(titles(shelf?.games ?? [])).toEqual(["From GOG never opened"]);
  });
});

describe("the proposal of Today", () => {
  it("the game that you were playing wins against anything to start", () => {
    // To propose something new while you have a game half done is what makes the
    // pile grow, which is exactly what this screen tries to undo.
    const library = [
      row({ title: "Never started" }),
      row({ title: "Half done", status: "playing", playtime_minutes: 300 }),
    ];
    expect(featured(library, NOW)?.game.title).toBe("Half done");
    expect(featured(library, NOW)?.reason).toBe("You have it half done");
  });

  it("between several started games, the one with the most recent game", () => {
    const library = [
      row({ title: "Old", status: "playing", last_played_at: NOW - 100 * DAY }),
      row({ title: "Recent", status: "playing", last_played_at: NOW - 2 * DAY }),
      row({ title: "With no date", status: "playing" }),
    ];
    expect(featured(library, NOW)?.game.title).toBe("Recent");
  });

  it("with nothing started, the selection changes with the day and not in the day", () => {
    // A proposal that changes each time that the screen is shown is a slot
    // machine, not a proposal.
    const library = [row({ title: "One" }), row({ title: "Two" }), row({ title: "Three" })];

    const today = featured(library, NOW)?.game.title;
    expect(featured(library, NOW + 3600)?.game.title).toBe(today!);

    const next = [1, 2, 3].map((d) => featured(library, NOW + d * DAY)?.game.title);
    expect(new Set([today, ...next]).size).toBe(3);
  });
});
