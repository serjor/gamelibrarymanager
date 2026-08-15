# 🎯 No component declares a colour: all of them come from the tokens

## 💡 Convention

All of the colours of the interface are defined **one time**, as CSS variables in
the `:root` of [`src/styles.css`](../../src/styles.css). Their dark variant is
defined **in the same place**, in the one `@media (prefers-color-scheme: dark)`
block. After that, no rule and no component writes a colour: it writes
`var(--token)`.

A colour that is not a token does not exist. That includes the colours that look
harmless:

- The literal values (`#b3261e`, `rgb(0 0 0 / .5)`).
- The system keywords (`Canvas`, `ButtonFace`), which change their value with the
  theme of the desktop and which you cannot compare with the tokens of this
  project.
- The colours derived from `currentColor`, which have one value or another value
  and the parent decides.

What you can do is to **derive** one token from another with `color-mix`, if you
also declare the result as a token or you use it in one rule only:

```css
--scrim: color-mix(in srgb, var(--text) 45%, transparent);
```

And there is one condition that makes you define the same token two times from
different source tokens: when the two themes need the same result — a darker
colour — but the dark token is not the same in each theme.

You can examine the rule quickly:

```sh
grep -nE '#[0-9a-fA-F]{3,6}' src/styles.css
```

It must give back lines only in the block where the tokens are defined.

## 🏆 Benefits

- **You change the dark theme in one place.** A `#b3261e` in the middle of the
  sheet makes you find it by hand on the day that you adjust the dark theme, and
  makes sure that you do not adjust one of them. With tokens, the dark block is
  the complete list of what to decide.
- **You can measure the contrast.** If the colour of the text and the colour of
  its background come from tokens, a test can read the two and calculate the
  ratio. With colours in many places, each place is a new condition.
- **The names say what each colour is for, not how it looks.**
  `--status-playing` survives a change of the blue; `--light-blue` does not.
- **To keep the accent apart from the semantic colours prevents a loud
  interface.** The accent shows where you are; the `--status-*` tokens say what
  each game is. When they mix, the screen shows four urgent things at the same
  time.

## 👀 Examples

### ✅ Good

```css
:root {
  --error: #b3261e;
  --status-playing: #1f6b86;
}

@media (prefers-color-scheme: dark) {
  :root {
    --error: #f2857b;
    --status-playing: #79c0dd;
  }
}

[role="alert"] {
  color: var(--error);
}

.status.playing {
  color: var(--status-playing);
}
```

The component does not know the colour of an error. It knows that it is an error.

### ❌ Bad

```css
[role="alert"] {
  color: #b3261e;
}

@media (prefers-color-scheme: dark) {
  [role="alert"] {
    color: #f2857b;
  }
}

/* And in a different place of the sheet, the bar that covers what goes below: */
.sticky {
  background: Canvas;
}
```

These are three problems in eleven lines. The colour of the error lives in two
rules that you must remember to change together; the dark block stops being the
list of what to decide and goes into all of the sheet; and `Canvas` is not the
background of the application but the background that the desktop decides, thus
the bar covers with a colour that is not the colour of the page.

## 🧐 Real world examples

- [`src/styles.css`](../../src/styles.css) defines all of the palette in `:root`
  and its dark variant in one block. That includes the semantic colours of the
  game status and the scrim of the record sheet.
- [`test/visual/look.ts`](../../test/visual/look.ts) measures the real contrast
  of the text and of the muted text against its background, in the light theme
  and in the dark theme, and against the background of the sheet, which is the
  only surface that is not drawn on the background of the page. It can do that
  exactly because the two colours come from tokens.
- The plan `.agents/plans/0002-rediseno-ui/plan.html` closes phase 1 with this
  rule as its criterion: the application had to look exactly the same after the
  change.

## 🔗 Related agreements

- [A test asserts on the structure, not on what it looks like](../testing/assert-on-the-structure.md)
  — why the contrast is calculated and is not seen in a screenshot.
- [One state for the two view modes](one-state-for-the-two-view-modes.md)
  — the same idea applied to state and not to colour: one source, many users.
- [`AGENTS.md`](../../AGENTS.md) — the index of all of the conventions.
