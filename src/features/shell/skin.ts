// A skin gives values to the tokens of `src/styles.css`, and nothing more.
//
// It is JSON and not CSS: a skin cannot hide a button, change a text or break
// the region that scrolls, and a contrast that is too low is found before the
// skin is applied. Each value goes to the document with `style.setProperty`,
// one property at a time, thus no value of a skin is ever CSS text.
//
// The format is a contract with persons outside this repository. A token that
// changes its name keeps its old name in `SKIN_TOKEN_ALIASES`. See the plan
// `.agents/plans/0009-skins/plan.html`, section "El formato de una skin".

export const SKIN_FORMAT = 1;

export type SkinMode = "light" | "dark";
export type SkinTokenKind = "color" | "shadow" | "length" | "number" | "duration" | "easing";
export type SkinFontSlot = "body" | "display" | "mono";

interface TokenSpec {
  /** `mode`: one value for light and one for dark. `shared`: one value for the two. */
  group: "mode" | "shared";
  kind: SkinTokenKind;
  /** Limits in rem for `length`, in ms for `duration`, as a number for `number`. */
  min?: number;
  max?: number;
}

const color = { group: "mode", kind: "color" } as const;
const shadow = { group: "mode", kind: "shadow" } as const;
const typeStep = { group: "shared", kind: "length", min: 0.5, max: 3 } as const;
const space = { group: "shared", kind: "length", min: 0, max: 4 } as const;
const radius = { group: "shared", kind: "length", min: 0, max: 1.5 } as const;
const leading = { group: "shared", kind: "number", min: 1, max: 2.5 } as const;
const motion = { group: "shared", kind: "duration", min: 0, max: 600 } as const;

/**
 * The tokens that a skin can give a value to. A test compares this list with
 * the `:root` block of `src/styles.css`: a new token fails it until somebody
 * decides here whether a skin can change it.
 */
export const SKIN_TOKENS: Readonly<Record<string, TokenSpec>> = {
  background: color,
  surface: color,
  "surface-raised": color,
  "surface-sunken": color,
  "surface-accent": color,
  "surface-selected": color,
  "surface-backlog": color,
  "surface-error": color,
  placeholder: color,
  border: color,
  "border-strong": color,
  text: color,
  "text-muted": color,
  accent: color,
  "accent-hover": color,
  "on-accent": color,
  "command-accent": color,
  "command-accent-hover": color,
  "archive-accent": color,
  "archive-accent-strong": color,
  "archive-surface": color,
  focus: color,
  error: color,
  scrim: color,
  "icon-frame": color,
  "icon-card-a": color,
  "icon-card-b": color,
  "icon-card-c": color,
  "art-pattern-a": color,
  "art-pattern-b": color,
  "art-pattern-c": color,
  "art-pattern-d": color,
  "status-backlog": color,
  "status-playing": color,
  "status-finished": color,
  "status-abandoned": color,
  "price-low": color,
  "t-xs": typeStep,
  "t-s": typeStep,
  "t-m": typeStep,
  "t-base": typeStep,
  "t-l": typeStep,
  "t-xl": typeStep,
  "t-2xl": typeStep,
  "leading-tight": leading,
  "leading-copy": leading,
  "e-1": space,
  "e-2": space,
  "e-3": space,
  "e-4": space,
  "e-5": space,
  "e-6": space,
  "e-7": space,
  "e-8": space,
  "radius-s": radius,
  "radius-m": radius,
  "radius-l": radius,
  "shadow-soft": shadow,
  "shadow-raised": shadow,
  "motion-fast": motion,
  "motion-base": motion,
  "motion-slow": motion,
  "motion-ease": { group: "shared", kind: "easing" },
};

/** The font tokens. A skin names them in `fonts`, not as tokens. */
export const SKIN_FONT_TOKENS: Readonly<Record<SkinFontSlot, string>> = {
  body: "font",
  display: "font-display",
  mono: "font-mono",
};

/** Old token name → current token name. Empty until a token changes its name. */
export const SKIN_TOKEN_ALIASES: Readonly<Record<string, string>> = {};

/**
 * The font families that the application bundles, with the stack that each one
 * becomes. A skin names a family from this list or `"system"`; it never brings
 * a font file.
 */
export const SKIN_FONT_FAMILIES: Readonly<Record<string, string>> = {
  "Press Start 2P": '"Press Start 2P", ui-monospace, monospace',
  VT323: '"VT323", ui-monospace, monospace',
  DotGothic16: '"DotGothic16", ui-monospace, monospace',
  Anton: '"Anton", Impact, sans-serif',
  "Archivo Narrow": '"Archivo Narrow", "Arial Narrow", sans-serif',
  Rajdhani: '"Rajdhani", "Arial Narrow", sans-serif',
  "JetBrains Mono": '"JetBrains Mono", ui-monospace, monospace',
  "IBM Plex Mono": '"IBM Plex Mono", ui-monospace, monospace',
  Saira: '"Saira", system-ui, sans-serif',
  Silkscreen: '"Silkscreen", ui-monospace, monospace',
  Cinzel: '"Cinzel", Georgia, serif',
  "EB Garamond": '"EB Garamond", Georgia, serif',
  "Pixelify Sans": '"Pixelify Sans", system-ui, sans-serif',
};

export interface Skin {
  format: typeof SKIN_FORMAT;
  id: string;
  name: string;
  author?: string;
  fonts: Partial<Record<SkinFontSlot, string>>;
  shared: Record<string, string>;
  light: Record<string, string>;
  dark: Record<string, string>;
}

export type SkinResult =
  | { ok: true; skin: Skin; warnings: string[] }
  | { ok: false; error: string };

/** The values that a skin does not give: those of `src/styles.css`, for each mode. */
export type SkinDefaults = Record<SkinMode, Readonly<Record<string, string>>>;

const TOP_LEVEL = new Set(["$schema", "format", "id", "name", "author", "fonts", "shared", "light", "dark"]);
const ID = /^[a-z0-9-]{1,40}$/;
const MAX_LABEL = 60;

// ---- Values ---------------------------------------------------------------

export type Rgba = [number, number, number, number];

const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const RGB = /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*(?:,\s*(0|1|0?\.\d+|1\.0+))?\s*\)$/;

/** Reads a colour in one of the forms that a skin accepts, or gives `null`. */
export function parseColor(value: string): Rgba | null {
  const text = value.trim();
  const hex = HEX.exec(text);
  if (hex) {
    let digits = hex[1]!;
    if (digits.length === 3) digits = [...digits].map((d) => d + d).join("");
    const byte = (i: number) => parseInt(digits.slice(i, i + 2), 16);
    return [byte(0), byte(2), byte(4), digits.length === 8 ? byte(6) / 255 : 1];
  }
  const rgb = RGB.exec(text);
  if (rgb) {
    const channels = [rgb[1], rgb[2], rgb[3]].map(Number);
    if (channels.some((c) => c > 255)) return null;
    return [channels[0]!, channels[1]!, channels[2]!, rgb[4] === undefined ? 1 : Number(rgb[4])];
  }
  return null;
}

const NUMBER = "-?(?:\\d+(?:\\.\\d+)?|\\.\\d+)";
const LENGTH = new RegExp(`^(${NUMBER})(px|rem)$`);

function lengthInRem(value: string): number | null {
  if (value === "0") return 0;
  const match = LENGTH.exec(value);
  if (!match) return null;
  const amount = Number(match[1]);
  return match[2] === "px" ? amount / 16 : amount;
}

const SHADOW_LENGTH = new RegExp(`^(?:0|${NUMBER}(?:px|rem))$`);

function isShadow(value: string): boolean {
  if (value === "none") return true;
  // One shadow: `[inset] x y [blur [spread]] colour`. The colour is the end,
  // and it can hold spaces and commas, thus it is taken first.
  const colourStart = value.search(/(#|rgba?\()/);
  if (colourStart <= 0) return false;
  if (parseColor(value.slice(colourStart)) === null) return false;
  const parts = value.slice(0, colourStart).trim().split(/\s+/);
  if (parts[0] === "inset") parts.shift();
  if (parts.length < 2 || parts.length > 4) return false;
  return parts.every((part) => SHADOW_LENGTH.test(part));
}

const EASING_KEYWORDS = new Set(["linear", "ease", "ease-in", "ease-out", "ease-in-out"]);
const BEZIER = new RegExp(`^cubic-bezier\\(\\s*(${NUMBER})\\s*,\\s*(${NUMBER})\\s*,\\s*(${NUMBER})\\s*,\\s*(${NUMBER})\\s*\\)$`);

function isEasing(value: string): boolean {
  if (EASING_KEYWORDS.has(value)) return true;
  const match = BEZIER.exec(value);
  if (!match) return false;
  const [x1, y1, x2, y2] = match.slice(1).map(Number) as [number, number, number, number];
  const inX = (x: number) => x >= 0 && x <= 1;
  const inY = (y: number) => y >= -2 && y <= 3;
  return inX(x1) && inX(x2) && inY(y1) && inY(y2);
}

function inLimits(amount: number, spec: TokenSpec): boolean {
  return (spec.min === undefined || amount >= spec.min) && (spec.max === undefined || amount <= spec.max);
}

/** Gives the reason why a value is refused, or `null` when the token accepts it. */
export function checkTokenValue(token: string, value: unknown): string | null {
  const spec = SKIN_TOKENS[token];
  if (spec === undefined) return `${token} is not a token`;
  if (typeof value !== "string") return `${token} must be a string`;
  const text = value.trim();
  switch (spec.kind) {
    case "color":
      return parseColor(text) ? null : `${token}: "${value}" is not a hex or rgb() colour`;
    case "shadow":
      return isShadow(text) ? null : `${token}: "${value}" is not one shadow with a hex or rgb() colour`;
    case "length": {
      const rem = lengthInRem(text);
      if (rem === null) return `${token}: "${value}" is not a length in px or rem`;
      return inLimits(rem, spec) ? null : `${token}: ${value} is outside ${spec.min}rem–${spec.max}rem`;
    }
    case "number": {
      const amount = new RegExp(`^${NUMBER}$`).test(text) ? Number(text) : Number.NaN;
      if (Number.isNaN(amount)) return `${token}: "${value}" is not a number`;
      return inLimits(amount, spec) ? null : `${token}: ${value} is outside ${spec.min}–${spec.max}`;
    }
    case "duration": {
      const match = /^(\d+)ms$/.exec(text);
      if (!match) return `${token}: "${value}" is not a duration in ms`;
      return inLimits(Number(match[1]), spec) ? null : `${token}: ${value} is outside ${spec.min}ms–${spec.max}ms`;
    }
    case "easing":
      return isEasing(text) ? null : `${token}: "${value}" is not an easing keyword or cubic-bezier()`;
  }
}

// ---- Contrast ---------------------------------------------------------------

function luminance([r, g, b]: Rgba): number {
  const channel = (v: number) => {
    const x = v / 255;
    return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function over(top: Rgba, bottom: Rgba): Rgba {
  const a = top[3];
  return [
    top[0] * a + bottom[0] * (1 - a),
    top[1] * a + bottom[1] * (1 - a),
    top[2] * a + bottom[2] * (1 - a),
    1,
  ];
}

/** The WCAG contrast ratio of two opaque colours. */
export function contrastRatio(a: Rgba, b: Rgba): number {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (high + 0.05) / (low + 0.05);
}

/**
 * The pairs that each mode must keep readable: text on its surfaces at 4.5:1
 * (WCAG AA), and the focus ring on the page at 3:1.
 */
export const CONTRAST_PAIRS: readonly { text: string; on: string; minimum: number }[] = [
  ...["text", "text-muted", "status-backlog", "status-playing", "status-finished", "status-abandoned"].flatMap(
    (text) => [
      { text, on: "background", minimum: 4.5 },
      { text, on: "surface", minimum: 4.5 },
    ],
  ),
  { text: "accent", on: "background", minimum: 4.5 },
  { text: "accent", on: "surface", minimum: 4.5 },
  { text: "accent-hover", on: "surface", minimum: 4.5 },
  { text: "archive-accent-strong", on: "background", minimum: 4.5 },
  { text: "on-accent", on: "accent", minimum: 4.5 },
  { text: "on-accent", on: "command-accent", minimum: 4.5 },
  { text: "on-accent", on: "command-accent-hover", minimum: 4.5 },
  { text: "error", on: "surface-error", minimum: 4.5 },
  { text: "price-low", on: "surface", minimum: 4.5 },
  { text: "focus", on: "background", minimum: 3 },
];

/**
 * Gives the first pair under its minimum, or `null`. A colour with alpha is
 * drawn on the page background first, as the interface draws it.
 */
export function checkContrast(values: Readonly<Record<string, string>>, mode: SkinMode): string | null {
  const page = parseColor(values.background ?? "");
  if (page === null) return `${mode} · background is missing`;
  if (page[3] < 1) return `${mode} · background must be opaque`;
  for (const pair of CONTRAST_PAIRS) {
    const surface = parseColor(values[pair.on] ?? "");
    const text = parseColor(values[pair.text] ?? "");
    if (surface === null || text === null) return `${mode} · ${pair.text} or ${pair.on} is missing`;
    const behind = over(surface, page);
    const ratio = contrastRatio(over(text, behind), behind);
    if (ratio < pair.minimum) {
      return `${mode} · ${pair.text} on ${pair.on} is ${ratio.toFixed(2)}:1, the minimum is ${pair.minimum}:1`;
    }
  }
  return null;
}

// ---- The skin -----------------------------------------------------------------

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function readTokens(
  block: unknown,
  group: TokenSpec["group"],
  where: string,
  warnings: string[],
): Record<string, string> | string {
  if (block === undefined) return {};
  if (!isRecord(block)) return `${where} must be an object`;
  const tokens: Record<string, string> = {};
  for (const [key, value] of Object.entries(block)) {
    const token = SKIN_TOKEN_ALIASES[key] ?? key;
    const spec = SKIN_TOKENS[token];
    if (spec === undefined) {
      warnings.push(`${where}.${key} is not a token of this version, and it is ignored`);
      continue;
    }
    if (spec.group !== group) {
      return `${where}.${key}: this token goes in ${spec.group === "mode" ? "light and dark" : "shared"}`;
    }
    const problem = checkTokenValue(token, value);
    if (problem !== null) return `${where}.${problem}`;
    tokens[token] = (value as string).trim();
  }
  return tokens;
}

function readLabel(value: unknown, field: string, required: boolean): string | undefined | { error: string } {
  if (value === undefined && !required) return undefined;
  if (typeof value !== "string" || value.trim() === "") return { error: `${field} must be a text` };
  if (value.length > MAX_LABEL) return { error: `${field} has more than ${MAX_LABEL} characters` };
  return value.trim();
}

/**
 * Reads the text of a skin file and examines all of it. `defaults` gives the
 * tokens that the skin does not change, for the contrast.
 */
export function validateSkin(text: string, defaults: SkinDefaults): SkinResult {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    return { ok: false, error: "the file is not JSON" };
  }
  if (!isRecord(data)) return { ok: false, error: "the file is not a JSON object" };
  if (data.format !== SKIN_FORMAT) {
    return { ok: false, error: `format ${JSON.stringify(data.format ?? null)} is not supported` };
  }
  if (typeof data.id !== "string" || !ID.test(data.id)) {
    return { ok: false, error: "id must be 1 to 40 characters: a–z, 0–9 and -" };
  }
  const name = readLabel(data.name, "name", true);
  if (isRecord(name)) return { ok: false, error: name.error as string };
  const author = readLabel(data.author, "author", false);
  if (isRecord(author)) return { ok: false, error: author.error as string };

  const warnings: string[] = [];
  for (const key of Object.keys(data)) {
    if (!TOP_LEVEL.has(key)) warnings.push(`${key} is not a field of format ${SKIN_FORMAT}, and it is ignored`);
  }

  const fonts: Skin["fonts"] = {};
  if (data.fonts !== undefined) {
    if (!isRecord(data.fonts)) return { ok: false, error: "fonts must be an object" };
    for (const [slot, family] of Object.entries(data.fonts)) {
      if (!(slot in SKIN_FONT_TOKENS)) {
        warnings.push(`fonts.${slot} is not a font slot, and it is ignored`);
        continue;
      }
      if (family !== "system" && (typeof family !== "string" || !(family in SKIN_FONT_FAMILIES))) {
        return { ok: false, error: `fonts.${slot}: "${String(family)}" is not a bundled font family` };
      }
      fonts[slot as SkinFontSlot] = family;
    }
  }

  const shared = readTokens(data.shared, "shared", "shared", warnings);
  if (typeof shared === "string") return { ok: false, error: shared };
  const modes = {} as Record<SkinMode, Record<string, string>>;
  for (const mode of ["light", "dark"] as const) {
    if (data[mode] === undefined) return { ok: false, error: `${mode} is missing: a skin defines the two modes` };
    const tokens = readTokens(data[mode], "mode", mode, warnings);
    if (typeof tokens === "string") return { ok: false, error: tokens };
    const contrast = checkContrast({ ...defaults[mode], ...tokens }, mode);
    if (contrast !== null) return { ok: false, error: contrast };
    modes[mode] = tokens;
  }

  const skin: Skin = { format: SKIN_FORMAT, id: data.id, name: name as string, fonts, shared, ...modes };
  if (author !== undefined) skin.author = author as string;
  return { ok: true, skin, warnings };
}

// ---- The document -------------------------------------------------------------

const ALL_PROPERTIES = [...Object.keys(SKIN_TOKENS), ...Object.values(SKIN_FONT_TOKENS)].map((t) => `--${t}`);

/**
 * Puts the values of a skin for one mode on `html`, and removes the values of
 * the skin before. `null` is the default skin: only `src/styles.css` decides.
 */
export function applySkin(skin: Skin | null, mode: SkinMode, root: HTMLElement = document.documentElement): void {
  for (const property of ALL_PROPERTIES) root.style.removeProperty(property);
  if (skin === null) return;
  const set = (token: string, value: string) => root.style.setProperty(`--${token}`, value);
  for (const [token, value] of Object.entries(skin.shared)) set(token, value);
  for (const [token, value] of Object.entries(skin[mode])) set(token, value);
  for (const [slot, family] of Object.entries(skin.fonts)) {
    const stack = SKIN_FONT_FAMILIES[family];
    if (stack !== undefined) set(SKIN_FONT_TOKENS[slot as SkinFontSlot], stack);
  }
}

/**
 * Reads the values of the default skin from the stylesheet that the document
 * has: the `:root` block and the dark block. Then `validateSkin` measures the
 * contrast against what the interface really draws.
 */
export function readSkinDefaults(doc: Document = document): SkinDefaults {
  const light: Record<string, string> = {};
  const dark: Record<string, string> = {};
  for (const sheet of [...doc.styleSheets]) {
    let rules: CSSRuleList;
    try {
      rules = sheet.cssRules;
    } catch {
      continue;
    }
    for (const rule of [...rules]) {
      if (!(rule instanceof CSSStyleRule)) continue;
      const target =
        rule.selectorText === ":root" ? light : rule.selectorText === ':root[data-theme="dark"]' ? dark : null;
      if (target === null) continue;
      for (const property of [...rule.style]) {
        if (property.startsWith("--")) target[property.slice(2)] = rule.style.getPropertyValue(property).trim();
      }
    }
  }
  return { light, dark: { ...light, ...dark } };
}
