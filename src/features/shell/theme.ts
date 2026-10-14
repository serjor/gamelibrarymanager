import { openPath } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { INCLUDED_SKINS } from "../../skins";
import { applySkin, readSkinDefaults, validateSkin, type Skin } from "./skin";
import { examineUserSkins, type SkinNotice, type UserSkin } from "./userSkins";

export const THEME_PREFERENCES = ["system", "light", "dark"] as const;
export type ThemePreference = (typeof THEME_PREFERENCES)[number];
export type EffectiveTheme = Exclude<ThemePreference, "system">;
export const THEME_STORAGE_KEY = "gamelibrarymanager.theme";

const DARK_MODE_QUERY = "(prefers-color-scheme: dark)";

/** The key of the stored skin. The default skin is the absence of a key. */
export const SKIN_STORAGE_KEY = "gamelibrarymanager.skin";

/** The value of the selector for the default skin, which is never stored. */
export const DEFAULT_SKIN_KEY = "default";

/**
 * The text of the active skin from the skins folder. The start reads it and
 * examines it again before React, thus the window does not paint Default and
 * then jump. The folder stays the source: when its list arrives, it decides.
 */
export const USER_SKIN_CACHE_KEY = "gamelibrarymanager.skin.user";

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

export function includedSkinKey(skin: Skin): string {
  return `builtin:${skin.id}`;
}

export function userSkinKey(skin: Skin): string {
  return `user:${skin.id}`;
}

/** Gives the skin of a stored key, or `null` (the default skin) when no skin has that key. */
export function findSkin(
  key: string | null,
  user: readonly Skin[] = [],
  included: readonly Skin[] = INCLUDED_SKINS,
): Skin | null {
  return (
    included.find((skin) => includedSkinKey(skin) === key) ??
    user.find((skin) => userSkinKey(skin) === key) ??
    null
  );
}

/** Makes a skin the active one. The next `applyTheme` puts it on the document. */
export function setActiveSkin(skin: Skin | null): void {
  activeSkin = skin;
}

export interface SkinChoice {
  key: string;
  name: string;
}

/** The options of the selector: the default skin, the included ones and the ones of the folder. */
export function skinChoices(
  user: readonly Skin[] = [],
  included: readonly Skin[] = INCLUDED_SKINS,
): { included: SkinChoice[]; user: SkinChoice[] } {
  return {
    included: [
      { key: DEFAULT_SKIN_KEY, name: "Default" },
      ...included.map((skin) => ({ key: includedSkinKey(skin), name: skin.name })),
    ],
    user: user.map((skin) => ({
      key: userSkinKey(skin),
      name: skin.author === undefined ? skin.name : `${skin.name} · ${skin.author}`,
    })),
  };
}

function storage(): Storage | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

/** Stores the key, and the text of a skin of the folder for the next start. */
export function persistSkinPreference(key: string, userText: string | null = null): boolean {
  const store = storage();
  if (store === null) return false;
  try {
    if (key === DEFAULT_SKIN_KEY) store.removeItem(SKIN_STORAGE_KEY);
    else store.setItem(SKIN_STORAGE_KEY, key);
    if (userText === null) store.removeItem(USER_SKIN_CACHE_KEY);
    else store.setItem(USER_SKIN_CACHE_KEY, userText);
    return true;
  } catch {
    return false;
  }
}

/** The skin of the folder that the last start used, examined again. */
function cachedUserSkin(key: string | null): Skin | null {
  if (key === null || !key.startsWith("user:")) return null;
  const text = storage()?.getItem(USER_SKIN_CACHE_KEY) ?? null;
  if (text === null) return null;
  const result = validateSkin(text, readSkinDefaults());
  return result.ok && userSkinKey(result.skin) === key ? result.skin : null;
}

export function bootstrapTheme(): EffectiveTheme {
  const key = readSkinPreference();
  setActiveSkin(findSkin(key) ?? cachedUserSkin(key));
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
  /** The value of the selector. */
  skin: string;
  onSkinChange: (key: string) => void;
  userSkins: readonly Skin[];
  /** The folder of the skins of other persons, or `null` until it is known. */
  skinsFolder: string | null;
  refusedSkins: readonly SkinNotice[];
  skinWarnings: readonly SkinNotice[];
  /** Why the stored skin is not applied, or `null`. */
  skinNotice: string | null;
  onReloadSkins: () => void;
  onOpenSkinsFolder: () => void;
}

function reapply(): void {
  if (typeof document === "undefined") return;
  // The skin changes and the mode does not: apply the mode that is there.
  applyTheme(document.documentElement.dataset.theme === "dark" ? "dark" : "light");
}

export function useSkinPreference(): SkinPreferenceState {
  // The stored key, also when its skin is not there: a file that goes away
  // for a moment does not change the choice of the user.
  const [stored, setStored] = useState<string | null>(() => readSkinPreference());
  const storedRef = useRef(stored);
  const [user, setUser] = useState<UserSkin[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [folder, setFolder] = useState<string | null>(null);
  const [refused, setRefused] = useState<SkinNotice[]>([]);
  const [warnings, setWarnings] = useState<SkinNotice[]>([]);
  const [folderError, setFolderError] = useState<string | null>(null);

  const onReloadSkins = useCallback(() => {
    api
      .listUserSkins()
      .then((result) => {
        const examined = examineUserSkins(result.files, readSkinDefaults());
        setFolder(result.dir);
        setUser(examined.skins);
        setRefused(examined.refused);
        setWarnings(examined.warnings);
        setFolderError(null);
        const key = storedRef.current;
        if (key !== null && key.startsWith("user:")) {
          const found = examined.skins.find(({ skin }) => userSkinKey(skin) === key);
          // The file decides: its new text, or Default while it is not there.
          setActiveSkin(found?.skin ?? null);
          if (found !== undefined) persistSkinPreference(key, found.text);
          reapply();
        }
      })
      .catch((cause: unknown) => setFolderError(errorMessage(cause)))
      .finally(() => setLoaded(true));
  }, []);

  useEffect(() => {
    onReloadSkins();
  }, [onReloadSkins]);

  const onSkinChange = useCallback(
    (key: string) => {
      const included = findSkin(key);
      const fromFolder = user.find(({ skin }) => userSkinKey(skin) === key);
      const next = included ?? fromFolder?.skin ?? null;
      const value = next === null ? DEFAULT_SKIN_KEY : key;
      storedRef.current = next === null ? null : key;
      setStored(storedRef.current);
      persistSkinPreference(value, fromFolder?.text ?? null);
      setActiveSkin(next);
      reapply();
    },
    [user],
  );

  const onOpenSkinsFolder = useCallback(() => {
    if (folder === null) return;
    openPath(folder).catch((cause: unknown) => setFolderError(errorMessage(cause)));
  }, [folder]);

  const userList = user.map(({ skin }) => skin);
  const active = findSkin(stored, userList);
  let skinNotice: string | null = folderError;
  if (skinNotice === null && loaded && active === null && stored !== null && stored.startsWith("user:")) {
    skinNotice = `The skin "${stored.slice("user:".length)}" is not in the skins folder or is not valid. Default is applied until it comes back.`;
  }

  return {
    skin: active === null ? DEFAULT_SKIN_KEY : (stored as string),
    onSkinChange,
    userSkins: userList,
    skinsFolder: folder,
    refusedSkins: refused,
    skinWarnings: warnings,
    skinNotice,
    onReloadSkins,
    onOpenSkinsFolder,
  };
}
