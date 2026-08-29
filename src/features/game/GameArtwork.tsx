import type { LibraryRow } from "../../lib/api";
import { artworkVariant, monogram } from "./artwork";

export type ArtworkSurface = "portrait" | "wide";

export interface GameArtworkProps {
  row: Pick<LibraryRow, "title" | "cover_url" | "store_cover_url">;
  surface?: ArtworkSurface;
  className?: string;
  loading?: "eager" | "lazy";
}

/**
 * Shows the artwork that belongs to a record.
 *
 * Portrait art comes from IGDB. Wide art comes from a store copy. Missing art
 * gets a deterministic monogram and token pattern, so the empty state remains
 * useful without inventing an image or downloading a filler asset.
 */
export function GameArtwork({
  row,
  surface = "portrait",
  className,
  loading,
}: GameArtworkProps) {
  const source = surface === "wide" ? row.store_cover_url : row.cover_url;
  const classes = [
    "game-artwork",
    `game-artwork--${surface}`,
    source ? "game-artwork--image" : "game-artwork--fallback",
    source ? undefined : `game-artwork--pattern-${artworkVariant(row.title)}`,
    className,
  ]
    .filter(Boolean)
    .join(" ");

  if (source) {
    return <img className={classes} src={source} alt="" loading={loading} />;
  }

  return (
    <span className={classes} aria-hidden="true">
      <span>{monogram(row.title)}</span>
    </span>
  );
}
