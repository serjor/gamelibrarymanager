# AGENTS.md

The index of the conventions of the project. Each convention lives in a file of
its own in `docs/`, in one directory for each area of the repository.

Before you write code, read also:

- [`README.md`](README.md) — the architecture, the crates and their boundaries.
- `.agents/plans/0001-game-library-manager/plan.html` — the agreed plan, with the
  closed decisions **and the alternatives refused**. Do not discuss them again:
  if you think that one of them is incorrect, say so and wait for an answer; do
  not change it.
- `.agents/plans/0002-rediseno-ui/plan.html` — the redesign of the interface,
  with the same rule. Beside it, `maquetas.html` shows the five alternatives that
  were compared, the four refused included.
- `.agents/plans/0003-english-migration/plan.html` — the migration of the code
  and the documentation to English. It records why the project writes in English
  with ASD-STE100, and the two consequences that reach outside the repository:
  the checksums of the migrations `0001` to `0005`, and the price format.
- `.agents/plans/0004-hardening-and-release/plan.html` — the eight phases that
  put a limit where there is none, add the two operations that a local-first
  application must have — to disconnect a store and to take the data out — and
  give the repository a way to publish. Each phase closes alone and gives a
  version of its own.
- [`docs/documentation-guidelines.md`](docs/documentation-guidelines.md) — how to
  write a new document and where it goes.

## Conventions

### `docs/connectors/` — the stores

| Convention | What it is about |
| --- | --- |
| [No store credential goes inside the binary](docs/connectors/credentials-outside-the-binary.md) | The user supplies all of it and it lives in the store of secrets, also when the secret is public. The application never asks for the password of a store. |
| [Verify the unofficial endpoints before you write the connector](docs/connectors/verify-unofficial-endpoints.md) | Read the live reference implementation, test by hand, and record the result with a date in the module. |
| [Every store connector has a switch of its own](docs/connectors/switch-per-connector.md) | You switch a broken store off and the others continue. The reason is kept, and only the user decides to switch a store off. |
| [An ambiguous identifier is not an identity](docs/connectors/an-ambiguous-identifier-is-not-an-identity.md) | If the store permits two readings of the identifier of the copy, the connector does not select: it gives no identifier and the title decides. |

### `docs/domain/` — the pure rules

| Convention | What it is about |
| --- | --- |
| [Money is kept in whole cents](docs/domain/money-in-cents.md) | Integers with their currency beside them, and text only when you show it. In floating point, 19.99 stops being 19.99 as soon as you calculate with it. |

### `docs/storage/` — the schema and the data

| Convention | What it is about |
| --- | --- |
| [To add metadata to a record writes its row again; it does not make a new one](docs/storage/enrich-records-in-place.md) | `user_state` is attached to the `game_id`: to use it again is what prevents the loss of what the user wrote. |
| [A price is a cache of the data of another person, and it is replaced complete](docs/storage/prices-are-a-cache-that-is-replaced.md) | The one exception to the logical delete, limited to two tables: an offer that ended cannot continue to look like an offer. |
| [A delete that compares against a list of the provider uses a temporary table](docs/storage/compare-against-a-temporary-table.md) | Never a `NOT IN` with thousands of placeholders, and never a comparison divided into batches: each batch would delete what is in the other batches. |

### `docs/tauri/` — the application shell

| Convention | What it is about |
| --- | --- |
| [Each interface link needs explicit scope in the capability](docs/tauri/url-scope-in-capabilities.md) | `opener:allow-open-url` enables the command but gives no scope, and the patterns are compared with no normalisation. |
| [What the webview needs from the environment goes in `main.rs`](docs/tauri/prepare-the-webview-before-gtk-starts.md) | Before GTK starts, behind a platform `cfg` and with respect for what the environment already gives. In the development script it corrects only the machine of the programmer. |
| [A script in a store login window runs on one page and carries no logic](docs/tauri/scripts-in-a-login-window.md) | You read the page of a store only where it gives the code, with no logic inside the script and with no command given to it. |
| [A long pass saves as it goes, and a provider that cuts it off is a result](docs/tauri/long-passes-save-in-batches.md) | Write in batches, stop where the provider stops you and lose nothing from before, and say why. A failure of the database does go up. |
| [A long operation takes a guard, and the guard clears the cancel flag](docs/tauri/one-long-operation-at-a-time.md) | One long operation at a time, and the second one is told that the application is busy. A command that goes away leaves no cancel behind, because the guard and not the command clears the flag. |

### `docs/ui/` — the interface

| Convention | What it is about |
| --- | --- |
| [No component declares a colour: all of them come from the tokens](docs/ui/tokens-as-the-only-source-of-colour.md) | The palette and its dark variant are defined one time. No literal values, no `Canvas`, and no colours derived from `currentColor`. |
| [One state for the two view modes: the views only show](docs/ui/one-state-for-the-two-view-modes.md) | The filter, the sort and the selection live in `Library.tsx`; the table and the wall show what they receive. And a screen that makes its own divisions is not a view mode. |
| [One region scrolls, and it reaches the edges of the window](docs/ui/one-region-that-scrolls.md) | The height is divided with `flex` and `min-height: 0`; the piece sets the maximum width and the frame does not, or the space at the sides stops answering the wheel. |

### `docs/testing/` — the tests

| Convention | What it is about |
| --- | --- |
| [A test asserts on the structure, not on what it looks like](docs/testing/assert-on-the-structure.md) | Count the statements and do not measure their time, assert on the plan of a query and not on its time, and measure the layout and do not look at it. A screenshot is not a test. |

### Areas with no convention yet

No area is empty. The next convention goes where it belongs.

## Checks

These are the checks that CI runs. All of them must pass before you close a
phase:

```sh
cargo fmt --all --check
cargo clippy --all-targets --workspace -- -D warnings
cargo test --workspace
bunx tsc --noEmit && bun run lint && bun test
bun run tauri dev            # the window must open
```

There are two checks that CI **cannot** make. The first needs a desktop session
with secret-service:

```sh
cargo test -p secrets --test keyring_real -- --ignored
```

The second needs a Chromium, because it measures the real layout — overlaps,
overflows, the alignment of columns and the contrast — and `bun test` does not
know that: with happy-dom it measures each container as zero:

```sh
bun run build && bun run visual
```

If it finds no browser: `bunx playwright install chromium`, or `CHROMIUM_PATH`
that points to the browser that you have. How to write a check of that kind is in
[assert on the structure](docs/testing/assert-on-the-structure.md).
