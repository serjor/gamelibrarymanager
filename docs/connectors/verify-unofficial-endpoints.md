# 🎯 Verify the unofficial endpoints before you write the connector

## 💡 Convention

Of the stores that this project reads, only Steam has a public API. What exists
for the others is community documentation, frequently written years ago. To
implement against it immediately is to write code against endpoints that can be
dead.

Before the first line of a connector, do two steps, in this order:

1. **Read the live reference implementation.** Heroic Games Launcher and gogdl
   for GOG, legendary for Epic. They are GPLv3 — which agrees with this licence
   — and, more important, somebody maintains them. Their commit history says
   what broke and when. A `git log` on the authentication file gives more than
   each wiki.
2. **Test the endpoints by hand.** One `curl` for each endpoint separates the
   three answers that are important: `200` is alive, `401` is alive and asks for
   credentials, `302` to a login screen is dead.

Only then do you implement, and **against what you examined**, not against what
the plan said.

Write the result of the examination **with the date** in the `//!` of the
module. An unofficial endpoint expires; to know when a person last looked at it
is one half of the diagnosis the next time that it breaks.

If the examination shows that all of the flow is impossible, apply the agreed
alternative and say so; do not invent a different path while you work. That one
endpoint has moved is **not** a reason for the alternative: it is a reason to
use the new endpoint.

## 🏆 Benefits

- You find what is broken before you spend the effort, not after.
- The date in the module turns "this no longer operates" into "this operated on
  14 August 2026, look at what has changed since then".
- To read the reference implementation teaches you the difficulties that the
  documentation does not give — which fields to filter, how the pages operate,
  what changes when you use it.
- Because all of them are GPLv3, you can take code if that becomes necessary.
  You must keep the headers and record it in [`NOTICE`](../../NOTICE).

## 👀 Examples

### ✅ Good

Examine and record, with the date and with the reason:

```rust
//! ## The endpoints (examined on 2026-08-14)
//!
//! The plan recorded endpoints from a 2018 dump and one half of them is no
//! longer applicable:
//!
//! - `auth.gog.com/auth` and `auth.gog.com/token` **continue to operate**. The
//!   token endpoint answers `invalid_grant` to a code that does not exist, which
//!   shows that it accepts the client and refuses only the code.
//! - `embed.gog.com/user/data/games` and
//!   `embed.gog.com/account/getFilteredProducts` are **dead**: they answer 302
//!   to the login screen. Heroic replaced them in its PR #5718 (June 2026) and
//!   keeps no reference to `embed.gog.com` in its library code.
//! - Today the library is read from
//!   `galaxy-library.gog.com/users/{id}/releases`, in pages through
//!   `page_token`.
```

The examination that gives those three lines:

```sh
# 302 to the login screen: dead.
curl -o /dev/null -w "%{http_code} %{redirect_url}\n" \
  https://embed.gog.com/user/data/games
# 401: alive, it only asks for credentials.
curl -o /dev/null -w "%{http_code}\n" \
  https://galaxy-library.gog.com/users/1/releases
# invalid_grant, not invalid_client: the client pair is still valid.
curl "https://auth.gog.com/token?client_id=…&client_secret=…&grant_type=authorization_code&code=INVALID"
```

And the difficulty that you see only if you read the reference:

```rust
// `platform_id` is more important than it looks: Galaxy also lists what the
// user has in other connected stores, thus without a filter here the GOG
// connector would create Steam copies that do not exist.
.filter(|item| item.owned && item.platform_id == PLATFORM_GOG)
```

### ❌ Bad

```rust
//! Endpoints from gogapidocs.
//! Library: embed.gog.com/user/data/games
```

No date, no examination, and against an endpoint that has given back a redirect
to the login for months. The connector compiles, the tests with invented
fixtures pass, and nothing operates against the real store.

```rust
// To assume the shape of the answer and not look at it: here `id` comes as a
// number, and to hold it as text with no conversion leaves the map empty
// quietly.
let id: String = product.id;
```

```rust
// "It answers 403, I will try to send the session cookie of the browser."
// To invent a path different from the agreed path as soon as the first fails.
```

## 🧐 Real world examples

- [`crates/connectors/src/gog/mod.rs`](../../crates/connectors/src/gog/mod.rs) —
  the `//!` with the dated examination and the result of each test.
- [`crates/connectors/src/gog/parse.rs`](../../crates/connectors/src/gog/parse.rs)
  — the filter by `platform_id` and the numeric `id` read as text: the two
  difficulties that came from Heroic and not from the documentation.
- [`crates/connectors/tests/gog.rs`](../../crates/connectors/tests/gog.rs) — the
  fixtures came from real answers of the day of the examination; the header of
  the file says so.
- [`NOTICE`](../../NOTICE) — it records Heroic and gogdl as the origin of the
  knowledge, with their licences, although no code was copied.
- The message of the commit `feat: fase 6` develops why the manual import
  alternative was **not** applied: the flow operated, only the list had moved.

## 🔗 Related agreements

- [No store credential goes inside the binary](credentials-outside-the-binary.md)
  — the second half of what you need to add a store.
- [Each interface link needs explicit scope in the capability](../tauri/url-scope-in-capabilities.md)
  — examine and do not assume, applied to the permissions.
