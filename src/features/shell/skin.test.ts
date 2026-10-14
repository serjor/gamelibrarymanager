import { afterEach, describe, expect, it } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  applySkin,
  checkContrast,
  checkTokenValue,
  contrastRatio,
  parseColor,
  SKIN_FONT_TOKENS,
  SKIN_TOKENS,
  validateSkin,
  type SkinDefaults,
} from "./skin";
import { applyTheme, findSkin, setActiveSkin } from "./theme";

const css = readFileSync(join(import.meta.dir, "../../styles.css"), "utf8");

function declarations(block: string): Record<string, string> {
  const tokens: Record<string, string> = {};
  for (const match of block.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    tokens[match[1]!] = match[2]!.trim();
  }
  return tokens;
}

const lightBlock = declarations(/\n:root \{([\s\S]*?)\n\}/.exec(css)![1]!);
const darkBlock = declarations(/\n:root\[data-theme="dark"\] \{([\s\S]*?)\n\s*\}/.exec(css)![1]!);
const defaults: SkinDefaults = { light: lightBlock, dark: { ...lightBlock, ...darkBlock } };

function skinText(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    format: 1,
    id: "test-skin",
    name: "Test skin",
    light: { accent: "#3157d5" },
    dark: { accent: "#91a7ff" },
    ...overrides,
  });
}

function refusal(text: string): string {
  const result = validateSkin(text, defaults);
  if (result.ok) throw new Error("the skin was accepted");
  return result.error;
}

afterEach(() => {
  setActiveSkin(null);
  document.documentElement.removeAttribute("style");
});

describe("the list of tokens", () => {
  it("is exactly the :root block of styles.css", () => {
    const known = [...Object.keys(SKIN_TOKENS), ...Object.values(SKIN_FONT_TOKENS)].sort();
    expect(known).toEqual(Object.keys(lightBlock).sort());
  });

  it("has a light and a dark value for each token that the dark block changes", () => {
    for (const token of Object.keys(darkBlock)) {
      expect({ token, group: SKIN_TOKENS[token]?.group }).toEqual({ token, group: "mode" });
    }
  });

  it("accepts the values of styles.css for each token", () => {
    for (const mode of ["light", "dark"] as const) {
      for (const token of Object.keys(SKIN_TOKENS)) {
        expect({ token, problem: checkTokenValue(token, defaults[mode][token]) }).toEqual({ token, problem: null });
      }
    }
  });

  it("keeps the contrast of the default skin in the two modes", () => {
    expect(checkContrast(defaults.light, "light")).toBeNull();
    expect(checkContrast(defaults.dark, "dark")).toBeNull();
  });
});

describe("the values", () => {
  it("reads the colour forms of the format", () => {
    expect(parseColor("#fff")).toEqual([255, 255, 255, 1]);
    expect(parseColor("#3157D5")).toEqual([49, 87, 213, 1]);
    expect(parseColor("#00000080")).toEqual([0, 0, 0, 128 / 255]);
    expect(parseColor("rgba(12, 19, 30, 0.58)")).toEqual([12, 19, 30, 0.58]);
    expect(parseColor("rgb(256, 0, 0)")).toBeNull();
  });

  it("refuses all that is not a value of the token", () => {
    const refused: [string, string][] = [
      ["background", "red"],
      ["background", "var(--text)"],
      ["background", "url(https://example.com/x.png)"],
      ["background", "#fff; color: red"],
      ["background", "#fff } body { display: none"],
      ["background", "expression(alert(1))"],
      ["background", "color-mix(in srgb, #fff 50%, #000)"],
      ["shadow-soft", "0 6px 18px url(x)"],
      ["shadow-soft", "0 6px 18px #000, 0 0 0 #000"],
      ["radius-m", "6"],
      ["radius-m", "50vw"],
      ["radius-m", "2rem"],
      ["t-base", "0.1rem"],
      ["e-5", "-1rem"],
      ["leading-copy", "9"],
      ["motion-base", "2s"],
      ["motion-base", "900ms"],
      ["motion-ease", "steps(4)"],
      ["motion-ease", "cubic-bezier(2, 0, 0, 1)"],
    ];
    for (const [token, value] of refused) {
      expect({ token, value, refused: checkTokenValue(token, value) !== null }).toEqual({ token, value, refused: true });
    }
  });

  it("accepts the other forms of each kind", () => {
    expect(checkTokenValue("shadow-soft", "none")).toBeNull();
    expect(checkTokenValue("shadow-soft", "inset 0 0 0 2px #000")).toBeNull();
    expect(checkTokenValue("radius-m", "0")).toBeNull();
    expect(checkTokenValue("radius-m", "0.5rem")).toBeNull();
    expect(checkTokenValue("motion-ease", "ease-out")).toBeNull();
  });
});

describe("validateSkin", () => {
  it("accepts a skin that changes some tokens", () => {
    const result = validateSkin(skinText({ author: "Ana", shared: { "radius-m": "0px" } }), defaults);
    expect(result).toEqual({
      ok: true,
      warnings: [],
      skin: {
        format: 1,
        id: "test-skin",
        name: "Test skin",
        author: "Ana",
        fonts: {},
        shared: { "radius-m": "0px" },
        light: { accent: "#3157d5" },
        dark: { accent: "#91a7ff" },
      },
    });
  });

  it("refuses a file that is not a skin of format 1", () => {
    expect(refusal("{")).toBe("the file is not JSON");
    expect(refusal("[]")).toBe("the file is not a JSON object");
    expect(refusal(skinText({ format: 2 }))).toBe("format 2 is not supported");
    expect(refusal(skinText({ id: "Bad Id" }))).toContain("id must be");
    expect(refusal(skinText({ name: "" }))).toBe("name must be a text");
    expect(refusal(skinText({ name: "x".repeat(61) }))).toContain("more than 60");
  });

  it("refuses a skin with one mode only", () => {
    expect(refusal(skinText({ dark: undefined }))).toBe("dark is missing: a skin defines the two modes");
  });

  it("refuses a token in the wrong group", () => {
    expect(refusal(skinText({ shared: { background: "#000000" } }))).toBe(
      "shared.background: this token goes in light and dark",
    );
    expect(refusal(skinText({ light: { "radius-m": "0px" } }))).toBe("light.radius-m: this token goes in shared");
  });

  it("refuses a value that is not of its token, and says which", () => {
    expect(refusal(skinText({ light: { accent: "url(x)" } }))).toBe(
      'light.accent: "url(x)" is not a hex or rgb() colour',
    );
  });

  it("refuses a skin with too little contrast, with the pair and the ratio", () => {
    const error = refusal(skinText({ dark: { text: "#0f141d" } }));
    expect(error).toStartWith("dark · text on background is 1.00:1");
    expect(error).toEndWith("the minimum is 4.5:1");
  });

  it("measures a colour with alpha as it is drawn on the page", () => {
    expect(refusal(skinText({ light: { text: "rgba(24, 34, 48, 0.2)" } }))).toStartWith(
      "light · text on background",
    );
  });

  it("refuses a font family that the application does not bundle", () => {
    expect(refusal(skinText({ fonts: { display: "Comic Sans MS" } }))).toBe(
      'fonts.display: "Comic Sans MS" is not a bundled font family',
    );
    expect(validateSkin(skinText({ fonts: { display: "system" } }), defaults).ok).toBe(true);
  });

  it("ignores with a warning what a later version can add", () => {
    const result = validateSkin(
      skinText({ images: { wall: "wall.png" }, light: { accent: "#3157d5", "glow-strong": "#fff" } }),
      defaults,
    );
    expect(result.ok && result.warnings).toEqual([
      "images is not a field of format 1, and it is ignored",
      "light.glow-strong is not a token of this version, and it is ignored",
    ]);
    expect(result.ok && result.skin.light).toEqual({ accent: "#3157d5" });
  });
});

describe("applySkin", () => {
  const skin = () => {
    const result = validateSkin(skinText({ shared: { "radius-m": "0px" } }), defaults);
    if (!result.ok) throw new Error(result.error);
    return result.skin;
  };

  it("puts the values of the mode on html, one property at a time", () => {
    const root = document.documentElement;
    applySkin(skin(), "dark");
    expect(root.style.getPropertyValue("--accent")).toBe("#91a7ff");
    expect(root.style.getPropertyValue("--radius-m")).toBe("0px");
    applySkin(skin(), "light");
    expect(root.style.getPropertyValue("--accent")).toBe("#3157d5");
  });

  it("removes all of its values for the default skin", () => {
    applySkin(skin(), "light");
    applySkin(null, "light");
    expect(document.documentElement.style.length).toBe(0);
  });

  it("follows the mode that the theme applies", () => {
    setActiveSkin(skin());
    applyTheme("dark");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#91a7ff");
    applyTheme("light");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#3157d5");
  });

  it("finds a stored skin by its key, included or from the folder", () => {
    const included = [skin()];
    expect(findSkin("builtin:test-skin", [], included)).toBe(included[0]!);
    expect(findSkin("user:test-skin", [], included)).toBeNull();
    expect(findSkin("user:test-skin", included, [])).toBe(included[0]!);
    expect(findSkin(null, included, included)).toBeNull();
  });
});

describe("contrastRatio", () => {
  it("gives 21:1 for black on white", () => {
    expect(contrastRatio([0, 0, 0, 1], [255, 255, 255, 1])).toBeCloseTo(21, 5);
  });
});
