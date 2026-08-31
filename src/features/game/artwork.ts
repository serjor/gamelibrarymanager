import type { ArtworkPattern } from "./artworkTypes";

const PATTERNS: ArtworkPattern[] = ["a", "b", "c", "d"];
const WORDS = /[\p{L}\p{N}]+/gu;

/**
 * Gets the title words that define a fallback artwork identity.
 *
 * Punctuation and spacing do not change the identity. Letters and numbers from
 * every writing system stay in the result, so a title does not need an English
 * translation to get a useful fallback.
 */
function titleWords(title: string): string[] {
  return title.normalize("NFKC").match(WORDS) ?? [];
}

/**
 * Creates a short, readable fallback label for a game title.
 *
 * One-word titles use their first two letters. Longer titles use the first
 * letter of the first and last words. A title with no letters or numbers uses
 * a question mark instead of rendering an empty cover.
 */
export function monogram(title: string): string {
  const words = titleWords(title);
  if (words.length === 0) return "?";

  const letters = Array.from(words[0] ?? "");
  const first = letters[0] ?? "";
  const lastWord = words.length === 1 ? words[0] : words.at(-1);
  const last = Array.from(lastWord ?? "")[0] ?? "";

  return (words.length === 1 ? first + (letters[1] ?? "") : first + last).toLocaleUpperCase("en");
}

/**
 * Selects one of four stable patterns for a title with no artwork.
 *
 * The hash uses normalized title text. It does not use time, randomness, or
 * object identity, so the same record keeps the same pattern across renders.
 */
export function artworkVariant(title: string): ArtworkPattern {
  const normalized = titleWords(title).join("").toLocaleLowerCase("en");
  let hash = 2_166_136_261;

  for (const character of normalized) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16_777_619);
  }

  return PATTERNS[(hash >>> 0) % PATTERNS.length] ?? "a";
}
