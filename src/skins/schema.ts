// The JSON Schema of the skin format, made from `SKIN_TOKENS`.
//
// The schema is for the editor of a skin author: it completes the names of the
// tokens and marks a value of the wrong form. It does not replace
// `validateSkin`, which also measures the contrast. A test makes sure that
// skin.schema.json is what this function gives. To write it again:
//
//     bun src/skins/schema.ts
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { SKIN_FONT_FAMILIES, SKIN_FONT_TOKENS, SKIN_FORMAT, SKIN_TOKENS, type SkinTokenKind } from "../features/shell/skin";

const PATTERNS: Record<SkinTokenKind, string> = {
  color: "^(#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})|rgba?\\(.*\\))$",
  shadow: "^(none|(inset\\s+)?.+\\s(#[0-9a-fA-F]{3,8}|rgba?\\(.*\\)))$",
  length: "^(0|-?\\d*\\.?\\d+(px|rem))$",
  number: "^-?\\d*\\.?\\d+$",
  duration: "^\\d+ms$",
  easing: "^(linear|ease|ease-in|ease-out|ease-in-out|cubic-bezier\\(.*\\))$",
};

function tokens(group: "mode" | "shared") {
  const properties: Record<string, object> = {};
  for (const [token, spec] of Object.entries(SKIN_TOKENS)) {
    if (spec.group === group) properties[token] = { type: "string", pattern: PATTERNS[spec.kind] };
  }
  return { type: "object", properties, additionalProperties: true };
}

export function skinSchema(): object {
  const family = { enum: ["system", ...Object.keys(SKIN_FONT_FAMILIES)] };
  return {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    title: "Game Library Manager skin",
    description: "A skin gives values to the tokens of the interface. See SKINS.md.",
    type: "object",
    required: ["format", "id", "name", "light", "dark"],
    properties: {
      $schema: { type: "string" },
      format: { const: SKIN_FORMAT },
      id: { type: "string", pattern: "^[a-z0-9-]{1,40}$" },
      name: { type: "string", minLength: 1, maxLength: 60 },
      author: { type: "string", minLength: 1, maxLength: 60 },
      fonts: {
        type: "object",
        properties: Object.fromEntries(Object.keys(SKIN_FONT_TOKENS).map((slot) => [slot, family])),
        additionalProperties: false,
      },
      shared: tokens("shared"),
      light: tokens("mode"),
      dark: tokens("mode"),
    },
  };
}

export function skinSchemaText(): string {
  return JSON.stringify(skinSchema(), null, 2) + "\n";
}

if (import.meta.main) {
  writeFileSync(join(import.meta.dir, "skin.schema.json"), skinSchemaText());
}
