# 🎯 A skin gives values to tokens, and never CSS

## 💡 Convention

A skin is a JSON file that gives values to the tokens of `src/styles.css`. It
gives nothing else: no selector, no rule, no `url()`, no font file.

Three rules follow from this:

1. **Each value goes through `validateSkin` and then `setProperty`.** The
   validator accepts only the form of its token (a colour, a length in limits,
   one shadow), and it measures the contrast. `applySkin` puts each value on
   `html` with `style.setProperty`, one property at a time. No value of a skin
   becomes CSS text in the document. That applies to the included skins too:
   a test examines them with the same validator.
2. **A new token goes in `SKIN_TOKENS`.** A test compares `SKIN_TOKENS` with the
   `:root` block of `src/styles.css`. When you add a token, the test fails
   until you decide its group (`mode` or `shared`) and its kind. Then write the
   schema again with `bun src/skins/schema.ts`.
3. **A token that changes its name keeps the old name in
   `SKIN_TOKEN_ALIASES`.** The names of the tokens are a contract with persons
   outside this repository. A skin written for the old name must continue to
   work.

## 🏆 Benefits

- **A skin cannot falsify the interface.** With CSS, a skin can hide a button,
  write a text with `content` or break the region that scrolls. With tokens, it
  can only change values that the interface already uses for that purpose.
- **The contrast is measured before the skin is applied.** The values are
  colours with a name and a role, thus the validator knows which colour is text
  and on which surface it is.
- **The stylesheet stays free to change.** The selectors and the classes are
  not a public API. Only the names of the tokens are, and the alias table
  protects them.
- **One validator for all skins.** The included skins have no short path. If
  one of them breaks a rule, the test fails, as it would fail for a skin of
  another person.

## 👀 Examples

### ✅ Good

```ts
// skin.ts: each value goes alone to its own property.
for (const [token, value] of Object.entries(skin[mode])) {
  root.style.setProperty(`--${token}`, value);
}
```

```json
{ "light": { "accent": "#5a1cc8" } }
```

### ❌ Bad

```ts
// A <style> with the text of the skin.
const style = document.createElement("style");
style.textContent = `:root { --accent: ${skin.light.accent}; }`;
document.head.append(style);
```

```json
{ "light": { "accent": "#5a1cc8; } .utility-actions { display: none" } }
```

With text CSS, one value that the validator does not see correctly closes the
block and adds a rule: here, the buttons of Utilities disappear. With
`setProperty`, the same value is a value of `--accent` that the browser
refuses, and the validator refuses it before that.

## 🧐 Real world examples

- [`src/features/shell/skin.ts`](../../src/features/shell/skin.ts) has
  `SKIN_TOKENS`, `SKIN_TOKEN_ALIASES`, `validateSkin` and `applySkin`.
- [`src/features/shell/skin.test.ts`](../../src/features/shell/skin.test.ts)
  compares `SKIN_TOKENS` with `src/styles.css` and refuses `url(`, `var(`, `;`
  and `}` in the values.
- [`src/skins/skins.test.ts`](../../src/skins/skins.test.ts) examines each
  included skin with `validateSkin`.
- [`src-tauri/src/commands/skins.rs`](../../src-tauri/src/commands/skins.rs)
  reads the skins folder and parses nothing: the validator stays in one place.
- [`SKINS.md`](../../SKINS.md) is the guide for the authors of skins.

## 🔗 Related agreements

- [No component declares a colour: all of them come from the tokens](tokens-as-the-only-source-of-colour.md)
  — the tokens are the only source of colour, and a skin only changes their
  values.
- [A test asserts on the structure, not on what it looks like](../testing/assert-on-the-structure.md)
  — why the contrast of a skin is calculated and not seen.
- [`AGENTS.md`](../../AGENTS.md) — the index of all of the conventions.
