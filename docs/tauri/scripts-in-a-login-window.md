# 🎯 A script in a store login window runs on one page and carries no logic

## 💡 Convention

Some stores do not hand the authorisation code back in the address. Epic answers
a JSON document, so the page has to be read, and reading a page means running a
script of ours inside a site that is not ours. That is allowed, under three
conditions that are not negotiable:

1. **One page only.** The script runs on the address that mints the code and
   nowhere else. The page where the user types their password is never touched.
   The check lives in Rust, before the script is even sent.
2. **No logic in the script.** It hands the text of the page over and stops.
   Whatever has to be understood is understood in Rust, where there are tests.
   The shape of that answer is exactly what will change the day the store moves,
   and a string of JavaScript is the one place where nothing would catch it.
3. **No commands for the remote page.** The login window is not listed in
   `capabilities/default.json`, so the page cannot invoke a single command of
   the application. It is deliberate and it stays that way.

When the address does carry the code —GOG— none of this applies: watching the
navigation is enough and no script is injected at all. Reading the page is the
exception, not the way logins are done here.

The window has one more duty: it must always end. A login resolves through the
right page **or** through the window being closed, and the two ways can fire
almost together, so the channel is resolved once and only once.

## 🏆 Benefits

- The password of the user never shares a page with code of this project, which
  is the promise the whole design of the login rests on.
- Parsing in Rust means the answer of the store has tests, fixtures and a
  compiler. In the script it would have none of the three.
- A window that cannot resolve twice is a window that cannot panic on a race,
  and a window that always resolves is a command that never hangs forever.
- Telling the three ways a login ends apart —code, unreadable page, closed
  window— is what lets the message say something useful instead of "failed".

## 👀 Examples

### ✅ Good

The script is one expression, and it only goes out on the right page:

```rust
const READ_BODY: &str = "document.body ? document.body.innerText : null";

.on_page_load(move |webview, payload| {
    if payload.event() != PageLoadEvent::Finished
        || !EpicConnector::is_authorization_page(payload.url().as_str())
    {
        return;
    }
    let _ = webview.eval_with_callback(READ_BODY, move |result| { /* … */ });
})
```

The understanding happens in Rust, and it is tested:

```rust
fn code_from_eval(result: &str) -> Option<String> {
    let body: String = serde_json::from_str(result).ok()?;
    EpicConnector::code_from_body(&body)
}
```

The ways out are told apart, so the message can be acted on:

```rust
enum Outcome {
    Code(String),
    Unreadable,
    Closed,
}
```

### ❌ Bad

```rust
// Injected on every page, including the one with the password field.
.on_page_load(move |webview, _| {
    let _ = webview.eval_with_callback(READ_BODY, /* … */);
})
```

```js
// Logic in the script: the shape of the answer of the store lives in a string
// that no test ever runs.
JSON.parse(document.body.innerText).authorizationCode || null
```

```rust
// One way out only. Closing the login window leaves the command hanging for
// ever, with no window and no error.
let code = rx.await;
```

```json
{
  "identifier": "default",
  "windows": ["main", "epic-login"]
}
```

Giving the login window a capability hands the commands of the application to a
remote page. There is no reason it would ever need them.

## 🧐 Real world examples

- [`src-tauri/src/commands/epic.rs`](../../src-tauri/src/commands/epic.rs) — the
  page check, the script with no logic, and the three ways a login ends.
- [`src-tauri/src/commands/gog.rs`](../../src-tauri/src/commands/gog.rs) — the
  same window without a single script, because GOG does carry the code in the
  address.
- [`crates/connectors/src/epic/mod.rs`](../../crates/connectors/src/epic/mod.rs)
  — `is_authorization_page` and `code_from_body`, which is where the tests are.
- [`src-tauri/capabilities/default.json`](../../src-tauri/capabilities/default.json)
  — `windows` names only `main`, and neither login window appears.

## 🔗 Related agreements

- [Todo enlace de la interfaz necesita alcance explícito en la capacidad](alcance-de-urls-en-capacidades.md)
  — least privilege applied to the permissions of the window.
- [Ninguna credencial de tienda va dentro del binario](../connectors/credenciales-fuera-del-binario.md)
  — the password of a store never passes through code of this project.
