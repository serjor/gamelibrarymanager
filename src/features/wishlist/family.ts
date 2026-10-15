import type { SteamFamilySettings } from "../../lib/api";

/**
 * The line that says what the family marks come from.
 *
 * The family is read in the synchronisation, thus the date is the date of the
 * last synchronisation that read it, and an error says why a later one did
 * not: the marks are the marks of that date, and the user must know it.
 */
export function familyLabel(settings: SteamFamilySettings): string {
  if (settings.refreshed_at === null) return "Steam Family not read yet";
  const when = new Date(settings.refreshed_at * 1000).toLocaleString();
  const read =
    settings.family_groupid === null
      ? `Steam says that you are in no family (read on ${when})`
      : `${settings.games} ${settings.games === 1 ? "game" : "games"} that your family can lend, read on ${when}`;
  return settings.last_error === null
    ? read
    : `${read}. The last synchronisation did not read it: ${settings.last_error}`;
}
