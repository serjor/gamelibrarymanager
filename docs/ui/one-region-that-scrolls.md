# 🎯 One region scrolls, and it reaches the edges of the window

## 💡 Convention

The window is the frame. Inside it there is **exactly one** box that scrolls —
the list, the queue or "Today" — and that box **reaches the two edges** of the
window.

Three concrete rules come from that:

- **The height is divided, not invented.** No `height: 70vh` for a list: the
  frame is `100%`, and the region keeps the space that stays after the header
  with `flex: 1`. Each link of that chain declares `min-height: 0`, because a
  flex child does not become smaller than its content unless you tell it to, and
  with the chain broken the list grows until it pushes the page.
- **The row that divides the space uses `align-items: stretch`.** With
  `flex-start`, the column is the size of its content and the `flex: 1` inside
  divides nothing: with eight games you do not see it and with four hundred it
  goes outside. A piece that must not stretch says so alone with `align-self`.
- **The piece sets the maximum width, not the frame.** A `max-width` on the
  container that scrolls leaves spaces at the sides that do not answer the wheel.
  Limit the table, the form or the card; the space that they leave is still part
  of the box that scrolls, thus the wheel operates over it.

## 🏆 Benefits

- **The wheel operates where the pointer is.** That is what a user expects, and
  that is what does not occur when the space at the sides belongs to a container
  that does not scroll.
- **One bar on the screen, not two.** Two bars make you look at which is which
  before you drag, and the external bar frequently has a very small movement: you
  move one finger and nothing occurs.
- **The header of the table and the bulk bar stay where they must.** With one
  region that scrolls, `position: sticky` has one context and does what it
  promises.
- **You can give a reason for the maximum of each piece separately.** The maximum
  of the table comes from a sum — 96rem, plus the space and the inspector, are
  the 120rem of a 1920 screen at its full size — and that makes an open record
  move no column.

## 👀 Examples

### ✅ Good

```css
html,
body,
#root {
  height: 100%;
}

main {
  height: 100%;          /* the frame does not scroll… */
  display: flex;
  flex-direction: column;
}

.library,
.library-body,
.library-main {
  flex: 1;
  min-height: 0;         /* …and all of the height goes down to the viewport */
}

.table-viewport {
  flex: 1;
  min-height: 0;
  overflow: auto;        /* …which is the only element that scrolls */
}

.table {
  max-width: 96rem;      /* the maximum goes here, not in `main` */
}
```

### ❌ Bad

```css
main {
  max-width: 80rem;
  margin: 0 auto;
}

.table-viewport {
  height: 70vh;
  overflow: auto;
}
```

On a 1920 screen this leaves 320 px dead at each side — the wheel over them finds
nothing to move — and the sum of the header plus the `70vh` plus the padding is
approximately forty pixels more than the window: a second bar appears, the bar of
the page, with forty pixels of movement. You must move the pointer to the centre
to make the list move.

## 🧐 Real world examples

- [`src/styles.css`](../../src/styles.css) divides the height from `html` to
  `.table-viewport` and `.wall-viewport`, and puts the maximums in `.table`,
  `.review`, `.featured` and `form`, never in `main`.
- [`test/visual/look.ts`](../../test/visual/look.ts) examines this in the three
  screens and at three widths: that neither the page nor the frame scrolls, that
  the region reaches the two edges and that it keeps a height that is usable.
  The last condition is the condition that found that the library stayed at 294
  px while the other two had 644.

## 🔗 Related agreements

- [A test asserts on the structure, not on what it looks like](../testing/assert-on-the-structure.md)
  — "there is one bar" is measured with a comparison of `scrollHeight` and
  `clientHeight`, not with a look at a screenshot, where an extra bar goes
  unseen.
- [One state for the two view modes](one-state-for-the-two-view-modes.md) — the
  table and the wall divide the height in the same way because they are attached
  to the same place.
- [`AGENTS.md`](../../AGENTS.md) — the index of all of the conventions.
