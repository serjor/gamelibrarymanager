import { openUrl } from "@tauri-apps/plugin-opener";
import { errorMessage, type LibraryRow } from "../../lib/api";

/**
 * "On Luna": the game is in the catalogue that Prime includes, and the click
 * opens its page on Luna.
 *
 * One component for the wishlist and the record, so that the two say the same
 * thing in the same words. It shows nothing when the row has no mark: with
 * Luna off, or for a game that is not in the catalogue, an absent badge says
 * the truth and an empty one would not.
 *
 * The address comes from Rust, built with the domain of the country and the
 * path that Luna gives. The capability permits `/game/**` on each Luna domain,
 * and the `capabilities` test examines each country.
 */
export function LunaLink({
  row,
  onError,
}: {
  row: LibraryRow;
  onError: (message: string) => void;
}) {
  const mark = row.luna;
  if (mark === undefined) return null;

  return (
    <button
      type="button"
      className="luna-mark"
      aria-label={`Play ${row.title} on Luna: it is included with Prime`}
      title={`In the Luna catalogue included with Prime as “${mark.title}”`}
      onClick={() => {
        openUrl(mark.url).catch((cause: unknown) =>
          onError(`Could not open ${mark.url}: ${errorMessage(cause)}`),
        );
      }}
    >
      On Luna ↗
    </button>
  );
}
