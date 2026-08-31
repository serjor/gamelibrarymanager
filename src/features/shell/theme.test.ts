import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createElement } from "react";
import {
  applyTheme,
  bootstrapTheme,
  persistThemePreference,
  readThemePreference,
  resolveEffectiveTheme,
  THEME_PREFERENCES,
  THEME_STORAGE_KEY,
  useThemePreference,
} from "./theme";

type MediaListener = (event: MediaQueryListEvent) => void;

let mediaMatches = false;
const mediaListeners = new Set<MediaListener>();
const media: MediaQueryList = {
  get matches() {
    return mediaMatches;
  },
  media: "(prefers-color-scheme: dark)",
  onchange: null,
  addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
    if (type === "change" && typeof listener === "function") {
      mediaListeners.add(listener as MediaListener);
    }
  },
  removeEventListener(type: string, listener: EventListenerOrEventListenerObject) {
    if (type === "change" && typeof listener === "function") {
      mediaListeners.delete(listener as MediaListener);
    }
  },
  addListener(listener: MediaListener) {
    mediaListeners.add(listener);
  },
  removeListener(listener: MediaListener) {
    mediaListeners.delete(listener);
  },
  dispatchEvent(event: Event) {
    for (const listener of mediaListeners) listener(event as MediaQueryListEvent);
    return true;
  },
} as MediaQueryList;

const originalMatchMedia = window.matchMedia;

function changeSystemTheme(prefersDark: boolean) {
  mediaMatches = prefersDark;
  act(() => {
    media.dispatchEvent(new Event("change"));
  });
}

function ThemeProbe() {
  const { theme, onThemeChange } = useThemePreference();
  return createElement(
    "button",
    { type: "button", onClick: () => onThemeChange("dark") },
    theme,
  );
}

describe("theme preference", () => {
  beforeEach(() => {
    mediaMatches = false;
    mediaListeners.clear();
    window.matchMedia = (() => media) as typeof window.matchMedia;
    window.localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  afterEach(() => {
    cleanup();
    window.matchMedia = originalMatchMedia;
    document.documentElement.removeAttribute("data-theme");
  });

  it("accepts the three stored preferences and uses System for invalid data", () => {
    for (const preference of THEME_PREFERENCES) {
      window.localStorage.setItem(THEME_STORAGE_KEY, preference);
      expect(readThemePreference()).toBe(preference);
    }

    window.localStorage.setItem(THEME_STORAGE_KEY, "solarized");
    expect(readThemePreference()).toBe("system");
  });

  it("uses a safe fallback when storage cannot be read or written", () => {
    const storage = window.localStorage;
    Object.defineProperty(window, "localStorage", {
      configurable: true,
      value: {
        getItem: () => {
          throw new Error("storage is unavailable");
        },
        setItem: () => {
          throw new Error("storage is unavailable");
        },
      } as unknown as Storage,
    });

    try {
      expect(readThemePreference()).toBe("system");
      expect(persistThemePreference("dark")).toBe(false);
    } finally {
      Object.defineProperty(window, "localStorage", { configurable: true, value: storage });
    }
  });

  it("resolves and applies the effective mode", () => {
    expect(resolveEffectiveTheme("system", false)).toBe("light");
    expect(resolveEffectiveTheme("system", true)).toBe("dark");
    expect(resolveEffectiveTheme("light", true)).toBe("light");
    expect(resolveEffectiveTheme("dark", false)).toBe("dark");

    applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("bootstraps an explicit stored mode before the application renders", () => {
    window.localStorage.setItem(THEME_STORAGE_KEY, "dark");
    expect(bootstrapTheme()).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("updates System when the operating-system mode changes and persists explicit choices", () => {
    render(createElement(ThemeProbe));
    expect(screen.getByRole("button").textContent).toBe("system");
    expect(document.documentElement.dataset.theme).toBe("light");

    changeSystemTheme(true);
    expect(document.documentElement.dataset.theme).toBe("dark");

    fireEvent.click(screen.getByRole("button"));
    expect(screen.getByRole("button").textContent).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");

    changeSystemTheme(false);
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
