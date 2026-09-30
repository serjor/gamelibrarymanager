import type { TimeToBeat } from "../../lib/api";

/**
 * The three durations in the order in which a player reads them, with the names
 * that HowLongToBeat made common. IGDB calls them `hastily`, `normally` and
 * `completely`, which says less to a person who wants to know whether a game
 * fits in a weekend.
 */
export const DURATIONS: { key: "hastily" | "normally" | "completely"; label: string }[] = [
  { key: "hastily", label: "Main story" },
  { key: "normally", label: "Main + extras" },
  { key: "completely", label: "Completionist" },
];

/**
 * Seconds to text, to the nearest half hour.
 *
 * A player estimate has no minutes of precision: "31 h 47 min" says more than
 * the data knows. Under one hour the minutes stay, because "0 h" is false.
 */
export function formatDuration(seconds: number | null): string {
  if (seconds === null || seconds <= 0) return "—";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${Math.max(minutes, 1)} min`;
  const halves = Math.round(minutes / 30);
  const whole = Math.floor(halves / 2);
  return halves % 2 === 0 ? `${whole} h` : `${whole}½ h`;
}

/** "1 player" and not "1 players": the count can be small, and then it matters. */
export function submissionsLabel(time: TimeToBeat): string {
  if (time.submissions <= 0) return "Estimate from IGDB";
  const players = time.submissions === 1 ? "player" : "players";
  return `Estimate from IGDB · ${time.submissions.toLocaleString("en")} ${players}`;
}
