# Write a skin

A skin changes how Game Library Manager looks: the colours, the fonts, the
corners, the shadows and the speed of the motion. It does not change the layout,
the texts or what a screen shows.

A skin is one JSON file. You put it in the skins folder, you click **Reload
skins** in Utilities, and you select it in **Skin**.

## The skins folder

The application makes the folder when it starts. **Open skins folder** in
Utilities opens it. Its place is in the data directory of the application:

| System | Folder |
| --- | --- |
| Linux | `~/.local/share/com.serjor.gamelibrarymanager/skins` |
| macOS | `~/Library/Application Support/com.serjor.gamelibrarymanager/skins` |
| Windows | `%APPDATA%\com.serjor.gamelibrarymanager\skins` |

The application reads only:

- The files that end in `.json`, in the folder itself. It does not read
  subfolders.
- Files of 64 KB or less, in UTF-8.
- The real file. A link is refused.
- The first 50 files, in the order of their names.

## A complete example

```json
{
  "$schema": "https://raw.githubusercontent.com/serjor/gamelibrarymanager/main/src/skins/skin.schema.json",
  "format": 1,
  "id": "neon-night",
  "name": "Neon Night",
  "author": "Ana",
  "fonts": { "display": "Press Start 2P" },
  "shared": { "radius-s": "0px", "radius-m": "0px", "radius-l": "2px" },
  "light": {
    "background": "#f4f1ff",
    "surface": "#ffffff",
    "text": "#1a1033",
    "text-muted": "#4d3f73",
    "accent": "#5a1cc8"
  },
  "dark": {
    "background": "#0b0618",
    "surface": "#140c2a",
    "text": "#efe8ff",
    "text-muted": "#b9acd9",
    "accent": "#b48cff"
  }
}
```

With `$schema`, an editor such as VS Code completes the names of the tokens and
marks a value of the wrong form.

## The fields

| Field | Rule |
| --- | --- |
| `format` | Necessary. It is `1`. |
| `id` | Necessary. 1 to 40 characters: `a`–`z`, `0`–`9` and `-`. Two files with the same `id` are both refused. |
| `name` | Necessary. Up to 60 characters. The selector shows it. |
| `author` | Optional. Up to 60 characters. The selector shows it after the name. |
| `fonts` | Optional. `body`, `display` (titles) and `mono` (figures). See [The fonts](#the-fonts). |
| `shared` | Optional. The tokens that are the same in the two modes. |
| `light`, `dark` | Necessary, both of them. The colours and the shadows of each mode. |

The keys of `shared`, `light` and `dark` are the names of the tokens with no
`--`. A token that you do not give keeps the value of the Default skin in that
mode. Give all of the colours if your background is far from the Default
background: a Default colour was not made for your background.

The application ignores a field or a token that it does not know, and it shows
a warning. A skin for a later version of the application thus still works,
with no new parts.

## The tokens

[`src/skins/skin.schema.json`](src/skins/skin.schema.json) has the complete
list, with the form of each value.

### `light` and `dark`: colours

A colour is `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)` or
`rgba(r, g, b, a)`. Nothing else: no names such as `red`, no `var()`, no
`color-mix()`, no `url()`.

| Tokens | What they colour |
| --- | --- |
| `background` | The page. It must be opaque. |
| `surface`, `surface-raised`, `surface-sunken` | Panels, raised panels such as menus, sunken areas. |
| `surface-accent`, `surface-selected` | A hovered row, a selected row. |
| `surface-backlog`, `surface-error` | The background of a backlog mark and of an error. |
| `placeholder` | A cover that has no image. |
| `border`, `border-strong` | Lines, and the lines of controls. |
| `text`, `text-muted` | Text and secondary text. |
| `accent`, `accent-hover`, `on-accent` | Links and the primary button; `on-accent` is the text on the button. |
| `command-accent`, `command-accent-hover` | The commands of the management screens. |
| `archive-accent`, `archive-accent-strong`, `archive-surface` | The collection screens: the wall of covers and the game record. |
| `focus` | The ring of the keyboard focus. |
| `error` | Error text. |
| `scrim` | What covers the screen behind a sheet. It can have alpha. |
| `icon-frame`, `icon-card-a`, `icon-card-b`, `icon-card-c` | The mark of the application. |
| `art-pattern-a` … `art-pattern-d` | The patterns of a cover with no image. |
| `status-backlog`, `status-playing`, `status-finished`, `status-abandoned` | The status of a game. |
| `price-low` | A price at its historical low. |

### `light` and `dark`: shadows

`shadow-soft` and `shadow-raised`: `none`, or one shadow:
`[inset] x y [blur [spread]] colour`, with lengths in `px` or `rem`.

### `shared`

| Tokens | Value |
| --- | --- |
| `t-xs`, `t-s`, `t-m`, `t-base`, `t-l`, `t-xl`, `t-2xl` | Text sizes. `px` or `rem`, from 0.5rem to 3rem. |
| `leading-tight`, `leading-copy` | Line height, a number from 1 to 2.5. |
| `e-1` … `e-8` | Spaces. `px` or `rem`, from 0 to 4rem. |
| `radius-s`, `radius-m`, `radius-l` | Corners. `px` or `rem`, from 0 to 1.5rem. |
| `motion-fast`, `motion-base`, `motion-slow` | Durations in `ms`, from 0 to 600. |
| `motion-ease` | `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out` or `cubic-bezier(x1, y1, x2, y2)`. |

Change the spaces and the text sizes with care. The table of the library has
rows of a fixed height.

## The fonts

A skin names a font family. It cannot bring a font file: the application does
not load fonts from other places. Each value is `"system"` (the font of
Default) or one of these families, which the application includes:

Press Start 2P, VT323, DotGothic16, Anton, Archivo Narrow, Rajdhani,
JetBrains Mono, IBM Plex Mono, Saira, Silkscreen, Cinzel, EB Garamond and
Pixelify Sans.

Keep `body` legible. A pixel font is good for `display`, the titles. In the
table of the library it is hard to read.

## The contrast

The application measures the contrast in each mode, with your values and the
Default values that you do not change. If a pair is below its minimum, the skin
is refused, and Utilities shows the pair and the ratio:

| Text | On | Minimum |
| --- | --- | --- |
| `text`, `text-muted`, each `status-*` | `background` and `surface` | 4.5:1 |
| `accent` | `background` and `surface` | 4.5:1 |
| `accent-hover` | `surface` | 4.5:1 |
| `archive-accent-strong` | `background` | 4.5:1 |
| `on-accent` | `accent`, `command-accent` and `command-accent-hover` | 4.5:1 |
| `error` | `surface-error` | 4.5:1 |
| `price-low` | `surface` | 4.5:1 |
| `focus` | `background` | 3:1 |

4.5:1 is the WCAG AA minimum for usual text.

## When a skin is not applied

Utilities lists each file that it does not apply, with the reason. If the skin
that you selected leaves the folder or stops being valid, the application
applies Default and keeps your choice. When the file comes back, click
**Reload skins**.

## Examples

The ten included skins are in [`src/skins/`](src/skins/). They use the same
format, and the application examines them as it examines yours.
