import { describe, expect, it } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { SkinDefaults } from "./skin";
import { examineUserSkins } from "./userSkins";

const css = readFileSync(join(import.meta.dir, "../../styles.css"), "utf8");
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

const skin = (id: string, extra: Record<string, unknown> = {}) =>
  JSON.stringify({ format: 1, id, name: id, light: {}, dark: {}, ...extra });

describe("examineUserSkins", () => {
  it("accepts the skins and says why the other files are not skins", () => {
    const result = examineUserSkins(
      [
        { file: "good.json", text: skin("good") },
        { file: "big.json", error: "it has more than 64 KB" },
        { file: "broken.json", text: "{" },
        { file: "pale.json", text: skin("pale", { light: { text: "#f5f1e8" } }) },
      ],
      defaults,
    );
    expect(result.skins.map((user) => [user.file, user.skin.id])).toEqual([["good.json", "good"]]);
    expect(result.refused).toEqual([
      { file: "big.json", message: "it has more than 64 KB" },
      { file: "broken.json", message: "the file is not JSON" },
      { file: "pale.json", message: "light · text on background is 1.00:1, the minimum is 4.5:1" },
    ]);
  });

  it("refuses the two files when two skins have one id", () => {
    const result = examineUserSkins(
      [
        { file: "a.json", text: skin("same") },
        { file: "b.json", text: skin("same") },
      ],
      defaults,
    );
    expect(result.skins).toEqual([]);
    expect(result.refused.map((notice) => notice.file)).toEqual(["a.json", "b.json"]);
  });

  it("keeps the text of the file for the next start, and its warnings", () => {
    const text = skin("later", { images: { wall: "wall.png" } });
    const result = examineUserSkins([{ file: "later.json", text }], defaults);
    expect(result.skins[0]?.text).toBe(text);
    expect(result.warnings).toEqual([
      { file: "later.json", message: "images is not a field of format 1, and it is ignored" },
    ]);
  });
});
