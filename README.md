# Game Library Manager

A desktop application that puts in one library the games that you have in
different stores. It gives them common metadata, it removes the duplicates, and
it lets you add your own backlog status.

Local-first: all of the data lives in a SQLite file on your machine. There is no
server, there are no accounts, and no store credential leaves your computer.

## Status

The eight phases of the plan are complete: Steam, GOG, Epic, IGDB metadata,
deduplication between stores, the backlog, and the prices of the wishlist with
IsThereAnyDeal.

You can start with Steam only. It is the one store with an official method, and
it needs your API key. You connect GOG and Epic on their own login page in the
application; your password does not come through here. Neither of the two lets
you register a third-party application. Thus you must also give the client pair
of their own launcher. That pair is public and the same for all users: the
application asks you for it so that it is not written inside the program.

Epic is the store with no public contract. It uses the private API of its
launcher, and it can stop to operate on the day that Epic decides. Thus each
connector has a switch of its own. If Epic breaks, you switch it off, and the
data that it gave stays in your library with your notes and your status.

The IGDB metadata is optional. Without it the library operates in the same way,
with the records made from the title of the store. That includes the
deduplication between stores. On the day that you configure IGDB, those records
get their metadata in place and keep the status that you wrote on them.

The prices are also optional, and for the same reason. With an ITAD key — which
is free — and your country, each wished-for game shows what it costs today, the
store that sells it, and the lowest price that it has had. The list is sorted by
discount. Without the key the list is still there, only with no prices. A button
of its own asks for the prices, and a synchronisation does not: a question to a
third party about what something costs must not prevent your Steam
synchronisation.

None of these credentials leaves your computer. They live in the keyring of the
system, or in an encrypted file if your desktop has no keyring.

The complete plan, with the decisions made and the alternatives refused, is in
[`.agents/plans/0001-game-library-manager/plan.html`](.agents/plans/0001-game-library-manager/plan.html).

## Development

You need stable Rust, [Bun](https://bun.sh) and, on Linux, the Tauri system
dependencies (`libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `librsvg2-dev`,
`libayatana-appindicator3-dev`).

```sh
bun install
bun run tauri dev
```

On Wayland, if the window stops at the start with `Error 71 (Protocol error)
dispatching to Wayland display`, the cause is the dmabuf renderer of WebKitGTK
and not the application:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 bun run tauri dev
```

These are the checks, the same checks that CI runs:

```sh
cargo fmt --check && cargo clippy --all-targets --workspace -- -D warnings && cargo test --workspace
bunx tsc --noEmit && bun run lint && bun test
```

## Architecture

| Crate | Responsibility |
| --- | --- |
| `crates/domain` | The entities and the rules. No network, no database, no Tauri. CI examines this. |
| `crates/storage` | SQLite and the migrations. All of the SQL of the project is here. |
| `crates/connectors` | The stores (Steam, GOG, Epic), only authentication and lists. Never downloads. |
| `crates/metadata` | The external providers: records (IGDB) and prices (ITAD). |
| `crates/secrets` | The native keyring of the operating system. |
| `src-tauri` | The application shell and the commands: they control, they do not decide. |
| `src` | The interface in React, in one directory for each feature. |

### Why this is a desktop application and not a web application

Steam is the one store of the three with an official method to read your
library. GOG and Epic have no public API, and you can defend their
authentication only if it runs on your machine. A server that kept those tokens
would disobey their terms of use, would be a target for an attack, and one IP
block would stop the service for all of the users at the same time. That is why
Playnite, Heroic and Lutris are desktop applications, and that is why this
application is one too.

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
