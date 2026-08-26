import type { LibraryRow } from "../../lib/api";

/**
 * The rules of "Today", pure and out of React, as the filter and the sort of the
 * library are.
 *
 * `now` comes in as a parameter and the clock is not read here: a rule that
 * looks at the time alone is not testable without you go through time, and one
 * half of the shelves depends on how long ago the last game was.
 */

const DAY = 86_400;
const SIX_MONTHS = 182 * DAY;

/** How many games go on a shelf. More than that, and the proposal is a list. */
const PER_SHELF = 12;

export interface Shelf {
  id: string;
  title: string;
  /** Why these games are there. A shelf with no reason is a list. */
  reason: string;
  games: LibraryRow[];
}

export interface Featured {
  game: LibraryRow;
  reason: string;
}

/** A live owned copy is necessary: a wished-for or removed record is not playable. */
function hasLiveCopy(row: LibraryRow): boolean {
  return row.owned_stores.length > 0;
}

function owned(rows: LibraryRow[]): LibraryRow[] {
  return rows.filter(hasLiveCopy);
}

/** Finished or abandoned is a decision already made, and "Today" does not open
 *  it again. */
function pending(row: LibraryRow): boolean {
  return row.status !== "finished" && row.status !== "abandoned";
}

function byTitle(a: LibraryRow, b: LibraryRow): number {
  return a.sort_title.localeCompare(b.sort_title, "en");
}

/**
 * The most recent first, and the games that publish no date last.
 *
 * Only Steam publishes the last time played, thus a GOG game comes here with no
 * date even if you played it yesterday. It goes last, which is what you can
 * declare; to hold it as "never played" would be to invent data.
 */
function byLastPlayed(a: LibraryRow, b: LibraryRow): number {
  if (a.last_played_at === null && b.last_played_at === null) return byTitle(a, b);
  if (a.last_played_at === null) return 1;
  if (b.last_played_at === null) return -1;
  return b.last_played_at - a.last_played_at;
}

/**
 * The shelves that have something in them, in the order in which they are shown.
 *
 * An empty shelf is not given back. To show "you have not touched it for a long
 * time" with no game below it tells you nothing and turns the screen into a list
 * of headings, which is the opposite of a proposal.
 */
export function shelves(rows: LibraryRow[], now: number): Shelf[] {
  const mine = owned(rows);

  const candidates: Shelf[] = [
    {
      id: "half-done",
      title: "You stopped in the middle",
      reason: 'Marked as "playing"',
      games: mine.filter((row) => row.status === "playing").sort(byLastPlayed),
    },
    {
      id: "not-touched",
      title: "You have not touched it for a long time",
      reason: "More than six months since the last game that the store publishes",
      games: mine
        .filter(
          (row) =>
            pending(row) && row.last_played_at !== null && now - row.last_played_at > SIX_MONTHS,
        )
        // The opposite of the others: the game that has waited the longest first.
        .sort((a, b) => (a.last_played_at ?? 0) - (b.last_played_at ?? 0)),
    },
    {
      id: "never-started",
      title: "Never started",
      reason: "In your library and with no game played",
      games: mine
        .filter((row) => pending(row) && row.playtime_minutes === 0 && row.last_played_at === null)
        .sort(byTitle),
    },
    {
      id: "two-times",
      title: "You have it two times",
      reason: "The same record with a copy in more than one store",
      games: mine.filter((row) => row.owned_stores.length > 1).sort(byTitle),
    },
  ];

  return candidates
    .filter((shelf) => shelf.games.length > 0)
    .map((shelf) => ({ ...shelf, games: shelf.games.slice(0, PER_SHELF) }));
}

/**
 * The game proposed today, with the reason for the proposal.
 *
 * The order of preference is the order of what is easiest to start again: the
 * game that you were playing always wins, because a proposal to start something
 * else while you have a game half done is exactly what makes the pile grow.
 *
 * When nothing is started, the selection changes with the day. The division uses
 * `now`, thus in the same day the result is always the same game — a proposal
 * that changes each time that the screen is shown is a slot machine — and on the
 * next day the result is a different game.
 */
export function featured(rows: LibraryRow[], now: number): Featured | null {
  const mine = owned(rows);

  const playing = mine.filter((row) => row.status === "playing").sort(byLastPlayed);
  if (playing[0]) {
    return { game: playing[0], reason: "You have it half done" };
  }

  const neverStarted = mine
    .filter((row) => pending(row) && row.playtime_minutes === 0)
    .sort(byTitle);
  if (neverStarted.length > 0) {
    return { game: ofTheDay(neverStarted, now), reason: "You have never started it" };
  }

  const rest = mine.filter(pending).sort(byTitle);
  if (rest.length > 0) {
    return { game: ofTheDay(rest, now), reason: "From the games that you have pending" };
  }

  return null;
}

function ofTheDay(games: LibraryRow[], now: number): LibraryRow {
  return games[Math.floor(now / DAY) % games.length]!;
}
