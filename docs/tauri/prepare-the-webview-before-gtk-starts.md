# 🎯 What the webview needs from the environment goes in `main.rs`, not in the script

## 💡 Convention

When the webview needs an environment variable to operate on a platform, that
variable goes in [`src-tauri/src/main.rs`](../../src-tauri/src/main.rs), in the
first lines of `main()` and **before GTK initialises**. Not in `package.json`,
not in the development script, not in the shell profile of the programmer.

Three more rules make sure that this does not become a place for everything:

- **Behind a `#[cfg(target_os = ...)]`.** A repair for Linux does not run on
  Windows or on macOS, where the webview engine is different.
- **Keep what comes from the environment.** Examine with `var_os` before you
  write, so that a user who wants the initial behaviour can ask for it with no
  new compile.
- **Record the date, the version and the symptom**, as with the unofficial
  endpoints. These repairs exist because of a defect of one engine with one
  driver: they are debt with an expiry date, and with no symptom written nobody
  will ever know whether you can remove them.

## 🏆 Benefits

- **It applies also to the packaged application.** A repair in the development
  script corrects only the machine of the programmer. The user who downloads the
  binary finds a window that does not open, and a user does not debug a window
  that does not open: they remove the application.
- **There is nothing to remember.** No variable in the profile, and no `README`
  with a "if you use Wayland, run this and not that".
- **The reason lives beside the code.** On the day that WebKitGTK corrects the
  defect, the comment says what to test to know whether the repair is
  unnecessary.
- **It is portable with no dependency.** A variable prefix before a command is
  not valid syntax in `cmd.exe`, thus to put it in `package.json` costs one more
  package only to start the application.

## 👀 Examples

### ✅ Good

```rust
/// Examined on 2026-08-15 in KDE on Wayland with `webkit2gtk-4.1` 2.52.5 and an
/// NVIDIA RTX 5070 Ti. Without the variable, the binary stops with code 1 and
/// `Gdk-Message: Error 71 (Protocol error)`; with the variable, the window
/// opens.
#[cfg(target_os = "linux")]
fn avoid_webkit_dmabuf() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    avoid_webkit_dmabuf();

    gamelibrarymanager_lib::run()
}
```

### ❌ Bad

```json
{
  "scripts": {
    "tauri": "WEBKIT_DISABLE_DMABUF_RENDERER=1 tauri"
  }
}
```

It corrects exactly one machine: the machine of the person who wrote it. The
binary that you distribute does not go through that script, thus the user with an
NVIDIA card on Wayland still sees the application close itself at the start, with
no message in any window. And the prefix does not operate on Windows, thus the
repair for one system breaks the start on another.

## 🧐 Real world examples

- [`src-tauri/src/main.rs`](../../src-tauri/src/main.rs) turns off the DMA-BUF
  renderer of WebKitGTK before the start, only on Linux and only if the
  environment does not say something different, with the exact symptom and the
  date of the examination.

## 🔗 Related agreements

- [Verify the unofficial endpoints before you write the connector](../connectors/verify-unofficial-endpoints.md)
  — the practice of a dated record comes from there: it is the same class of
  debt, on something that this project does not control and that will change.
- [Each interface link needs explicit scope in the capability](url-scope-in-capabilities.md)
  — the other thing that you find only when you run the real application.
- [A test asserts on the structure, not on what it looks like](../testing/assert-on-the-structure.md)
  — this repair was selected with a comparison of the binary with the variable
  and without it, and not with an assumption about the cause.
