# 🎯 Every store connector has a switch of its own

## 💡 Convention

A store that stops answering must never make the application useless. Each
connector can be switched off on its own, and switching it off leaves everything
the others brought exactly where it was.

Three parts, and all three are needed:

1. **A switch that lasts.** `connector_state` keeps whether a store is on. A
   store with no row is on and healthy, so the table only grows when something
   happens.
2. **A reason that outlives the run.** The synchronisation writes down why a
   store failed and clears it when the store answers again. A report shown once
   is gone the next time the application opens, and the library the user is
   looking at is still short of a store.
3. **Isolation while it runs.** One store failing is one entry in
   `failures`, never an error that ends the pass. What is switched off is not
   asked for anything, not even a token, and it is named in `skipped`.

Switching off is a decision of the user, so nothing else may take it. A failure
does **not** switch a connector off by itself —the next run may work— and a
store that answers again does **not** switch itself back on.

The reason this exists is Epic. Steam has a documented API and GOG at least has
a stable one; Epic rests on the private API of its own launcher and can change
on a day nobody chose. The switch is what turns that from "the application
broke" into "one store is off".

## 🏆 Benefits

- A broken store costs one line in the interface instead of a failed
  synchronisation every time.
- The kept reason is the difference between "Epic did not come back" and "the
  library is missing games and nobody knows why".
- Not asking a switched off store anything means no waiting on a request that is
  known to fail, and no rate limit spent on it.
- The user gets a way out that does not involve deleting the account: what Epic
  already brought stays in the library, with its state and its notes.

## 👀 Examples

### ✅ Good

The switch is read before anything is asked of the store:

```rust
// A switched off connector is not asked anything, not even for a token.
if disabled.contains(&account.store) {
    report.skipped.push(account.store.as_str().to_owned());
    continue;
}
```

The reason is written down, not only reported:

```rust
match result {
    Ok(()) => connectors.record_error(account.store, None).await?,
    Err(error) => {
        let reason = error.to_string();
        connectors.record_error(account.store, Some(&reason)).await?;
        report.failures.push(SyncFailure { /* … */ reason });
    }
}
```

And the message says what to do, because it is what the user will read:

```rust
#[error("invalid or expired credentials: connect the account again")]
Unauthorized,
```

### ❌ Bad

```rust
// One store brings down the pass. Steam and GOG synchronised fine and the user
// ends up with nothing.
for account in accounts {
    sync_account(&db, secrets, connector, &account, &mut report).await?;
}
```

```rust
// Switching it off by itself on the first failure. A network hiccup, a token
// that expired mid flight, and the store is off until somebody notices.
if result.is_err() {
    connectors.set_enabled(account.store, false).await?;
}
```

```rust
// Turning it back on because it answers again. The user switched it off on
// purpose and the application undoes the decision behind their back.
connectors.set_enabled(account.store, true).await?;
```

```rust
// A reason that lives only in the report. Reopening the application leaves a
// library that is missing a store and no trace of why.
report.failures.push(SyncFailure { reason: error.to_string(), /* … */ });
```

## 🧐 Real world examples

- [`migrations/0006_connector_switch.up.sql`](../../migrations/0006_connector_switch.up.sql)
  — the table, and why a missing row means on and healthy.
- [`crates/storage/src/repositories/connector_state.rs`](../../crates/storage/src/repositories/connector_state.rs)
  — `record_error` never touches `enabled`, which is what keeps the decision of
  the user out of reach of the algorithm.
- [`src-tauri/src/sync.rs`](../../src-tauri/src/sync.rs) — `sync_stores` reads
  the switch before the loop and writes the reason after each account.
- [`src-tauri/tests/connector_switch.rs`](../../src-tauri/tests/connector_switch.rs)
  — a broken Epic next to a working Steam, which is the "done when" of phase 7.
- [`crates/storage/tests/connector_state.rs`](../../crates/storage/tests/connector_state.rs)
  — recovering from an error does not switch a connector back on.
- [`src/App.tsx`](../../src/App.tsx) — only the connectors with something to say
  are shown; a store that works does not appear at all.

## 🔗 Related agreements

- [Verify the unofficial endpoints before you write the connector](verify-unofficial-endpoints.md)
  — the same problem seen earlier: what has no public contract will move.
- [No store credential goes inside the binary](credentials-outside-the-binary.md)
  — the other half of what you need to add a store.
