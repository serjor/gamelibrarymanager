import { useCallback, useEffect, useState } from "react";
import { INCLUDED_SKINS } from "../../skins";
import { applySkin, type Skin } from "./skin";

export const THEME_PREFERENCES = ["system", "light", "dark"] as const;
export type ThemePreference = (typeof THEME_PREFERENCES)[number];
export type EffectiveTheme = Exclude<ThemePreference, "system">;
export const THEME_STORAGE_KEY = "gamelibrarymanager.theme";

const DARK_MODE_QUERY = "(prefers-color-scheme: dark)";

/** The key of the stored skin. The default skin is the absence of a key. */
export const SKIN_STORAGE_KEY = "gamelibrarymanager.skin";

/** The value of the selector for the default skin, which is never stored. */
export const DEFAULT_SKIN_KEY = "default";

// The skin goes with the mode: each change of mode applies the values of the
// skin for the new mode.
let activeSkin: Skin | null = null;

function isThemePreference(value: string | null): value is ThemePreference {
  return value === "system" || value === "light" || value === "dark";
}

function systemMediaQuery(): MediaQueryList | null {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return null;
  }
  return window.matchMedia(DARK_MODE_QUERY);
}

export function readThemePreference(): ThemePreference {
  try {
    if (typeof window === "undefined") return "system";
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isThemePreference(stored) ? stored : "system";
  } catch {
    return "system";
  }
}

export function persistThemePreference(preference: ThemePreference): boolean {
  try {
    if (typeof window === "undefined") return false;
    window.localStorage.setItem(THEME_STORAGE_KEY, preference);
    return true;
  } catch {
    return false;
  }
}

export function resolveEffectiveTheme(
  preference: ThemePreference,
  prefersDark: boolean,
): EffectiveTheme {
  if (preference === "system") return prefersDark ? "dark" : "light";
  return preference;
}

export function applyTheme(theme: EffectiveTheme): void {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.theme = theme;
  applySkin(activeSkin, theme);
}

export function readSkinPreference(): string | null {
  try {
    if (typeof window === "undefined") return null;
    return window.localStorage.getItem(SKIN_STORAGE_KEY);
  } catch {
    return null;
  }
}

/** Gives the skin of a stored key, or `null` (the default skin) when no skin has that key. */
export function findSkin(key: string | null, skins: readonly Skin[] = INCLUDED_SKINS): Skin | null {
  return skins.find((skin) => `builtin:${skin.id}` === key) ?? null;
}

/** Makes a skin the active one. The next `applyTheme` puts it on the document. */
export function setActiveSkin(skin: Skin | null): void {
  activeSkin = skin;
}

/** The options of the selector: the default skin first, then the included ones. */
export function skinChoices(skins: readonly Skin[] = INCLUDED_SKINS): { key: string; name: string }[] {
  return [
    { key: DEFAULT_SKIN_KEY, name: "Default" },
    ...skins.map((skin) => ({ key: `builtin:${skin.id}`, name: skin.name })),
  ];
}

export function persistSkinPreference(key: string): boolean {
  try {
    if (typeof window === "undefined") return false;
    if (key === DEFAULT_SKIN_KEY) window.localStorage.removeItem(SKIN_STORAGE_KEY);
    else window.localStorage.setItem(SKIN_STORAGE_KEY, key);
    return true;
  } catch {
    return false;
  }
}

export function bootstrapTheme(): EffectiveTheme {
  setActiveSkin(findSkin(readSkinPreference()));
  const preference = readThemePreference();
  const media = systemMediaQuery();
  const theme = resolveEffectiveTheme(preference, media?.matches ?? false);
  applyTheme(theme);
  return theme;
}

export interface ThemePreferenceState {
  theme: ThemePreference;
  onThemeChange: (preference: ThemePreference) => void;
}

export function useThemePreference(): ThemePreferenceState {
  const [theme, setTheme] = useState<ThemePreference>(() => readThemePreference());

  const onThemeChange = useCallback((preference: ThemePreference) => {
    setTheme(preference);
    persistThemePreference(preference);
    const media = systemMediaQuery();
    applyTheme(resolveEffectiveTheme(preference, media?.matches ?? false));
  }, []);

  useEffect(() => {
    const media = systemMediaQuery();
    applyTheme(resolveEffectiveTheme(theme, media?.matches ?? false));
    if (media === null) return;

    const onChange = () => {
      if (theme === "system") {
        applyTheme(resolveEffectiveTheme(theme, media.matches));
      }
    };

    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", onChange);
      return () => media.removeEventListener("change", onChange);
    }

    media.addListener(onChange);
    return () => media.removeListener(onChange);
  }, [theme]);

  return { theme, onThemeChange };
}

export interface SkinPreferenceState {
  skin: string;
  onSkinChange: (key: string) => void;
}

export function useSkinPreference(): SkinPreferenceState {
  const [skin, setSkin] = useState<string>(() => {
    const key = readSkinPreference();
    return findSkin(key) === null ? DEFAULT_SKIN_KEY : (key as string);
  });

  const onSkinChange = useCallback((key: string) => {
    const next = findSkin(key);
    const stored = next === null ? DEFAULT_SKIN_KEY : key;
    setSkin(stored);
    persistSkinPreference(stored);
    setActiveSkin(next);
    if (typeof document === "undefined") return;
    // The skin changes and the mode does not: apply the mode that is there.
    applyTheme(document.documentElement.dataset.theme === "dark" ? "dark" : "light");
  }, []);

  return { skin, onSkinChange };
}
