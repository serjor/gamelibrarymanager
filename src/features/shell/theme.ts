import { useCallback, useEffect, useState } from "react";

export const THEME_PREFERENCES = ["system", "light", "dark"] as const;
export type ThemePreference = (typeof THEME_PREFERENCES)[number];
export type EffectiveTheme = Exclude<ThemePreference, "system">;
export const THEME_STORAGE_KEY = "gamelibrarymanager.theme";

const DARK_MODE_QUERY = "(prefers-color-scheme: dark)";

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
}

export function bootstrapTheme(): EffectiveTheme {
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
