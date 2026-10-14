import { describe, expect, it } from "bun:test";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { SKIN_FONT_FAMILIES, SKIN_TOKENS, validateSkin, type SkinDefaults } from "../features/shell/skin";
import { INCLUDED_SKINS } from ".";
import { skinSchemaText } from "./schema";

const root = join(import.meta.dir, "..");
const css = readFileSync(join(root, "styles.css"), "utf8");
const fontsCss = readFileSync(join(root, "fonts.css"), "utf8");
const fontsDir = join(root, "assets/fonts");

function declarations(block: string): Record<string, string> {
  const tokens: Record<string, string> = {};
  for (const match of block.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/g)) tokens[match[1]!] = match[2]!.trim();
  return tokens;
}

const light = declarations(/\n:root \{([\s\S]*?)\n\}/.exec(css)![1]!);
const defaults: SkinDefaults = {
  light,
  dark: { ...light, ...declarations(/\n:root\[data-theme="dark"\] \{([\s\S]*?)\n\s*\}/.exec(css)![1]!) },
};

describe("the included skins", () => {
  it("are ten, with different ids", () => {
    expect(new Set(INCLUDED_SKINS.map((skin) => skin.id)).size).toBe(10);
  });

  for (const skin of INCLUDED_SKINS) {
    it(`${skin.id} passes the examination of a skin from another person`, () => {
      const result = validateSkin(readFileSync(join(import.meta.dir, `${skin.id}.json`), "utf8"), defaults);
      expect(result).toEqual({ ok: true, skin, warnings: [] });
    });

    it(`${skin.id} gives each colour in the two modes`, () => {
      // A skin that gives only some colours takes the rest from the default
      // palette, which was not made for its background.
      const modeTokens = Object.keys(SKIN_TOKENS).filter((token) => SKIN_TOKENS[token]!.group === "mode");
      expect(Object.keys(skin.light).sort()).toEqual(modeTokens.sort());
      expect(Object.keys(skin.dark).sort()).toEqual(modeTokens.sort());
    });
  }
});

describe("the bundled fonts", () => {
  const faces = [...fontsCss.matchAll(/font-family: "([^"]+)";\s*src: url\("\.\/([^"]+)"\)/g)];

  it("have a @font-face for each family that a skin can name, and no other", () => {
    expect([...new Set(faces.map((face) => face[1]))].sort()).toEqual(Object.keys(SKIN_FONT_FAMILIES).sort());
  });

  it("point to files that exist, each with its OFL.txt beside it", () => {
    for (const [, , path] of faces) {
      expect(statSync(join(root, path!)).isFile()).toBe(true);
      expect(readFileSync(join(root, path!, "../OFL.txt"), "utf8")).toContain("SIL OPEN FONT LICENSE");
    }
  });

  it("weigh no more than 2 MB in total", () => {
    let total = 0;
    for (const family of readdirSync(fontsDir)) {
      for (const file of readdirSync(join(fontsDir, family))) total += statSync(join(fontsDir, family, file)).size;
    }
    expect(total).toBeLessThanOrEqual(2 * 1024 * 1024);
  });

  it("are only the fonts that an included skin uses", () => {
    const used = new Set(INCLUDED_SKINS.flatMap((skin) => Object.values(skin.fonts)));
    used.delete("system");
    expect([...used].sort()).toEqual(Object.keys(SKIN_FONT_FAMILIES).sort());
  });
});

describe("skin.schema.json", () => {
  it("is what the list of tokens gives", () => {
    expect(readFileSync(join(import.meta.dir, "skin.schema.json"), "utf8")).toBe(skinSchemaText());
  });
});
