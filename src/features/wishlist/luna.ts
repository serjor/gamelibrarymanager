import type { LunaSettings } from "../../lib/api";

/**
 * How old the Luna catalogue can be before the wishlist asks for it again.
 *
 * Luna rotates its catalogue each month, thus one day is far more often than
 * it changes. The point is the other direction: a user who opens the wishlist
 * every day never sees a catalogue older than a day, with no button to press.
 */
export const LUNA_STALE_SECONDS = 24 * 60 * 60;

/**
 * Whether the wishlist must ask for the catalogue when it opens.
 *
 * With Luna off there is nothing to ask. With no refresh yet, it asks: that is
 * a country kept by a refresh that failed after it, which the next attempt can
 * repair.
 */
export function lunaIsStale(settings: LunaSettings | null, nowSeconds: number): boolean {
  if (settings === null) return false;
  if (settings.refreshed_at === null) return true;
  return nowSeconds - settings.refreshed_at >= LUNA_STALE_SECONDS;
}

/**
 * The line that says which catalogue the marks come from.
 *
 * Amazon selects the catalogue from the connection and not from the country
 * that the user selected. When the two differ, the line says so: otherwise a
 * user in Germany with a Spanish connection would read "DE" over a Spanish
 * catalogue.
 */
export function lunaCatalogueLabel(settings: LunaSettings): string {
  if (settings.refreshed_at === null) return "Luna catalogue not read yet";
  const when = new Date(settings.refreshed_at * 1000).toLocaleString();
  const territory = settings.territory ?? settings.country;
  const differs =
    settings.territory !== null && settings.territory !== settings.country
      ? ` (Amazon gave the catalogue of ${settings.territory}, not of ${settings.country}: it decides from your connection)`
      : "";
  return `Luna catalogue of ${territory}, read on ${when}${differs}`;
}
