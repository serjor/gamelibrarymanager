# 🎯 Each interface link needs explicit scope in the capability

## 💡 Convention

For the interface to open an address in the browser, you need **two things**, not
one:

1. The **command**: `opener:allow-open-url`.
2. The **scope**: an `allow` list with patterns that agree with that address.

`opener:allow-open-url` alone enables the command *"without any pre-configured
scope"*, thus with no list **all** of the addresses are refused with `Not allowed
to open url`.

And `glob::Pattern` compares the patterns against **the string exactly as the
interface gives it**: the plugin does not normalise the URL, it does not add the
last slash and it does not change it in any way. A pattern must agree character
by character.

Thus:

- The code lists the concrete addresses. It does **not** use
  `opener:allow-default-urls`, which opens all of `http://` and `https://`.
- Each pattern is written in the same way as the interface constant that will use
  it.
- The wildcard is permitted in the **path**, never in the **host**:
  `https://www.gog.com/game/*` limits the scope, `https://*.gog.com` limits
  nothing.

There are two kinds of address and the tests examine them differently, because
you cannot examine them in the same way:

| Origin | Example | How the test examines it |
| --- | --- | --- |
| An interface constant | the page of the Steam key | The test finds it in `src/` and demands that a pattern permits it |
| Built with data | the page of a game in its store | There is no literal to find: the test uses real examples |

The test [`capabilities`](../../src-tauri/tests/capabilities.rs) does **not**
examine whether there are extra patterns, although that would be symmetrical. To
go through the connectors to find that is not usable: their literals are mostly
endpoints that the program *calls* — `https://api.gog.com` — and not pages that
the user *opens*, and from outside you cannot tell them apart. To permit one by
mistake would be worse than the problem to resolve. What the test does demand,
and what really protects against scope given away, is that no pattern carries a
wildcard in the host.

Also, the `catch` of an `openUrl` **contains the cause**. To hide the cause turns
an incorrect permission into a mystery.

## 🏆 Benefits

- The first-start screens operate. With no scope, the links "Get my key" and
  "Find my SteamID" were broken during all of phase 3 and nobody saw it: in a
  container with no browser nobody clicks them.
- Minimum scope is real scope. With `allow-default-urls`, each piece of code that
  arrives in the webview can open each page; with four addresses listed, it
  cannot.
- The test closes a complete class of defect that no other check of the project
  saw: neither `tsc`, nor `eslint`, nor `clippy`, nor the Rust suite compares a
  capability JSON against TypeScript constants.
- An error that contains its cause is an error that you diagnose when you read
  it. Without the cause you must find the permission by hand.

## 👀 Examples

### ✅ Good

```json
{
  "identifier": "opener:allow-open-url",
  "comment": "The patterns are compared with the string exactly as the interface gives it, with no normalisation, thus they must agree character by character with the constants of src/features/onboarding/.",
  "allow": [
    { "url": "https://steamcommunity.com/dev/apikey" },
    { "url": "https://steamid.io" },
    { "url": "https://www.igdb.com/games/*" },
    { "url": "https://store.steampowered.com/app/*" }
  ]
}
```

And the constant is written complete, so that the test can find it:

```tsx
// To add a constant to a value leaves `https://www.igdb.com/games/` in the code,
// which is what the test looks for.
const IGDB_GAME_URL = "https://www.igdb.com/games/";
open(IGDB_GAME_URL + candidate.slug);
```

```tsx
// The cause goes with the message: it is what turns "it does not open" into
// "Not allowed to open url", which already says where to look.
openUrl(url).catch((cause: unknown) =>
  setError(`Could not open ${url}: ${errorMessage(cause)}`),
);
```

### ❌ Bad

```json
{
  "permissions": ["core:default", "opener:allow-open-url"]
}
```

It enables the command and permits no address: **all** of the links fail.

```json
{ "url": "https://steamid.io/*" }
```

It does not agree with `https://steamid.io`. The pattern demands a literal `/`
after the host, the interface constant does not have one, and the plugin does not
add it.

```json
{ "url": "https://*" }
```

It is equal to `allow-default-urls`: scope given away to save four lines.

```tsx
// To interpolate the complete address leaves no constant to find: the test
// cannot examine it and the defect becomes invisible again.
open(`https://www.igdb.com/games/${candidate.slug}`);
```

```tsx
// It hides the reason and leaves the user — and the person who debugs it — with
// nothing.
openUrl(url).catch(() => setError(`Could not open ${url}`));
```

## 🧐 Real world examples

- [`src-tauri/capabilities/default.json`](../../src-tauri/capabilities/default.json)
  — the four addresses of the setup screens, with the reason in the `comment`
  field.
- [`src-tauri/tests/capabilities.rs`](../../src-tauri/tests/capabilities.rs) — it
  takes the `https://` constants of `src/features/` and examines that a pattern
  permits each one. It reads them from the code and does not keep a separate
  list, because a separate list becomes old exactly when it is important. The
  addresses built with data go with examples, and a third test prohibits the
  wildcard in the host.
- [`src/features/review/ReviewQueue.tsx`](../../src/features/review/ReviewQueue.tsx)
  — `IGDB_GAME_URL` is the constant that makes a link built with the slug of each
  candidate possible to find.
- [`src/features/onboarding/SteamSetup.tsx`](../../src/features/onboarding/SteamSetup.tsx),
  [`IgdbSetup.tsx`](../../src/features/onboarding/IgdbSetup.tsx) and
  [`GogSetup.tsx`](../../src/features/onboarding/GogSetup.tsx) — the constants
  that the test reads, and the `catch` that carries the cause.

## 🔗 Related agreements

- [Verify the unofficial endpoints before you write the connector](../connectors/verify-unofficial-endpoints.md)
  — the same idea applied outside: examine and do not assume.
- [No store credential goes inside the binary](../connectors/credentials-outside-the-binary.md)
  — the second half of the chapter about permissions.
