import { openUrl } from "@tauri-apps/plugin-opener";
import { errorMessage, type LibraryRow } from "../../lib/api";

/**
 * "Steam Family": a member of the Steam family of the user owns the game and
 * can lend it, and the click opens its page in the Steam store.
 *
 * One component for the wishlist and the record, as `LunaLink`, so that the
 * two say the same thing in the same words. It shows nothing when the row has
 * no mark: with the family off, or for a game that nobody in it owns.
 *
 * The names are the names of the profiles of the owners. When Steam did not
 * give them, the mark still says that the family can lend the game, and only
 * cannot say who.
 */
export function FamilyLink({
  row,
  onError,
}: {
  row: LibraryRow;
  onError: (message: string) => void;
}) {
  const mark = row.steam_family;
  if (mark === undefined) return null;
  const owners = mark.owners.join(", ");

  return (
    <button
      type="button"
      className="family-mark"
      aria-label={
        owners === ""
          ? `Your Steam family can lend you ${row.title}`
          : `${owners} can lend you ${row.title} with Steam Family`
      }
      title={`In the Steam library of your family as “${mark.title}”`}
      onClick={() => {
        openUrl(mark.url).catch((cause: unknown) =>
          onError(`Could not open ${mark.url}: ${errorMessage(cause)}`),
        );
      }}
    >
      {owners === "" ? "Steam Family ↗" : `Steam Family · ${owners} ↗`}
    </button>
  );
}
