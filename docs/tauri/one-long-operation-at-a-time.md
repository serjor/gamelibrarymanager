# 🎯 A long operation takes a guard, and the guard clears the cancel flag

## 💡 Convention

This application has three long commands: the synchronisation, the prices and
the matching. All three walk over hundreds of items, all three take minutes, and
all three read one cancel flag to know when the user asked them to stop.

Two rules, and they are one rule seen from two sides:

1. **A long command takes the right to run before it starts.** The right is one
   for the whole application. A command that does not get it stops immediately
   with `AppError::Busy` and tells the user that something else runs. It does
   not wait in a queue: a button that answers minutes later reads as an
   application that stopped.
2. **The guard clears the cancel flag, and the command does not.** The command
   holds the guard to its last line, and the guard clears the flag when it goes
   away.

The second rule is the one that is easy to get wrong. A command is a future, and
Tauri drops the future of a command whose window went away. A command that
clears the flag itself never reaches that line, thus:

- the flag stays set, and the next operation stops at its first safe point with
  nobody who asked for it;
- the right stays taken, and no long operation runs again until the application
  restarts.

Do not put the rule in the interface. Buttons that disable each other are what
the user sees, not what protects the state: a second window, a keyboard, or a
command sent by hand does not obey them.

## 🏆 Benefits

- A cancel reaches the operation that the user sees, because only one runs.
- A window that closes in the middle of a synchronisation leaves the same state
  as a synchronisation that ended: no cancel, and the right free.
- The user reads "another long operation is in progress" instead of watching two
  operations cancel each other with no message.
- Two commands cannot write the same rows at the same time, which is what a
  synchronisation and a matching would do over `game_link`.

## 👀 Examples

### ✅ Good

The command takes the guard, and gives it no further thought:

```rust
pub async fn sync_now(app: AppHandle, state: State<'_, AppState>) -> Result<SyncReport, AppError> {
    // The guard lives to the end of the command, and it is what clears the
    // cancel flag when the command goes away.
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    let progress = WindowProgress { app: app.clone(), state: &state };
    sync::sync_all(&state, &progress).await
}
```

The right is taken without a wait, and the flag is cleared in `Drop`:

```rust
pub fn try_begin(&self) -> Option<OperationGuard<'_>> {
    let right = self.operation.try_lock().ok()?;
    // The cancel of the operation before is not the cancel of this one.
    self.cancel_flag.store(false, Ordering::Relaxed);
    Some(OperationGuard { cancel_flag: &self.cancel_flag, _right: right })
}

impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        self.cancel_flag.store(false, Ordering::Relaxed);
    }
}
```

### ❌ Bad

```rust
// The command clears the flag. A window that closes never gets here: the flag
// stays set, and the next operation stops at its first safe point.
state.begin_operation();
let report = sync::sync_all(&state, &progress).await;
state.end_operation();
report
```

```rust
// Nothing prevents the second operation. The two share the flag, thus the
// second clears the cancel of the first, and the first to end clears the flag
// under the other.
state.begin_operation();
```

```rust
// To wait for the right. The button of the user answers when the operation
// before it ends, which can be five minutes, and the window says nothing.
let guard = state.operation.lock().await;
```

```rust
// To leave the rule in the interface alone. It is what the user sees, and it
// is not what keeps the state correct.
<button disabled={busy !== null}>Synchronise</button>
```

## 🧐 Real world examples

- [`src-tauri/src/state.rs`](../../src-tauri/src/state.rs) — `try_begin`,
  `OperationGuard` and its `Drop`.
- [`src-tauri/src/commands/mod.rs`](../../src-tauri/src/commands/mod.rs) — the
  same one line in `sync_now`, `refresh_prices` and `resolve_identities`.
- [`src-tauri/src/error.rs`](../../src-tauri/src/error.rs) — `AppError::Busy`,
  with the message that the user reads.
- [`src-tauri/tests/one_operation.rs`](../../src-tauri/tests/one_operation.rs) —
  a second operation gives `Busy`, and a command that is dropped where it waits
  leaves the flag clear and the right free.

## 🔗 Related agreements

- [A long pass saves as it goes, and a provider that cuts it off is a result](long-passes-save-in-batches.md)
  — what the operation does while it holds the guard, and why a cancel is cheap.
- [Every store connector has a switch of its own](../connectors/switch-per-connector.md)
  — the same idea for a store: one part that stops does not stop the others.
