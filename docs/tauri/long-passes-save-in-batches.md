# 🎯 A long pass saves as it goes, and a provider that cuts it off is a result

## 💡 Convention

Three of the use cases of this application walk over hundreds of items and talk
to somebody else's server on every one: the synchronisation, the matching and
the prices. All three take minutes, and all three will be interrupted. What they
must never do is throw away the work they had already done.

Four rules, and they are one rule seen from four sides:

1. **Write in chunks, not at the end.** A pass that only writes on its last line
   is a pass that loses everything to one 429. The matching writes every 25
   games; the synchronisation writes every account; the prices write every game.
2. **A provider that cuts you off is a result, not an error.** The pass stops
   where it was cut, keeps what it had, and returns the reason inside the
   report. What does rise as an error is a failure that stops the writing
   itself, which is the database: if nothing can be written, there is nothing to
   save.
3. **Say why it stopped.** A report nobody paints is a report that does not
   exist. A half done pass with no explanation is worse than an error, because
   the user cannot tell it apart from a pass that had nothing to do.
4. **Pressing again continues.** Every pass is idempotent and reads its pending
   work from the database, so it never needs to remember where it was.

## 🏆 Benefits

- Five minutes of a rate limited provider stop costing five minutes of work.
- The user gets a way forward that is one click, instead of a run that has to
  start from zero and will hit the same limit at the same place.
- Cancelling becomes cheap, so a slow pass can be interrupted without a price,
  and that is what makes the cancel button honest.
- A crash, a closed window or a kill leaves the same state as a cancel: the last
  chunk, and nothing half written.

## 👀 Examples

### ✅ Good

The provider stops the pass; the database does not:

```rust
let decision = match decide(igdb, credentials, token, &entry).await {
    Ok(decision) => decision,
    // A stop from the provider stops the pass at this point, and the earlier
    // work is still kept. A failure of the database goes up: if you cannot
    // write, there is nothing to keep.
    Err(AppError::Metadata(error)) => {
        report.stopped = Some(error.to_string());
        break;
    }
    Err(other) => return Err(other),
};
```

The chunk, with the reason it can be repeated:

```rust
since_last_save += 1;
if since_last_save == BATCH {
    // `rebuild_auto` writes the same set of links each time, thus twenty calls
    // give the same result as one call.
    GameLinkRepository(db).rebuild_auto(&links).await?;
    since_last_save = 0;
}
```

And the interface says it out loud:

```tsx
if (stopped !== null) {
  setError(
    `The matching stopped: ${stopped}. The work made to that point is ` +
      'kept; click "Match" again to continue from there.',
  );
}
```

### ❌ Bad

```rust
// To write only at the end. A 429 at game three hundred leaves the database
// exactly as it was, after five minutes of waiting.
for entry in pending {
    let decision = decide(igdb, credentials, token, &entry).await?;
    links.push(/* … */);
}
GameLinkRepository(db).rebuild_auto(&links).await?;
```

```rust
// To hide the reason. The pass gives back zero matches and the user cannot
// tell "there was nothing to do" from "IGDB stopped me".
Err(_) => break,
```

```rust
// To hold a failure of the database as a stop from the provider. The pass
// continues and writes in something that accepts no write.
Err(_) => { report.stopped = Some(error.to_string()); break; }
```

```rust
// To keep each item in a transaction of its own "for safety". One thousand
// transactions where forty are sufficient: that is what the batch is for.
for entry in pending {
    GameLinkRepository(db).rebuild_auto(&links).await?;
}
```

## 🧐 Real world examples

- [`src-tauri/src/identity.rs`](../../src-tauri/src/identity.rs) — `BATCH`, and
  the `match` that separates a stop from the provider from a failure of the
  database.
- [`src-tauri/tests/identity.rs`](../../src-tauri/tests/identity.rs) — a 429 in
  the middle keeps the work before the stop, with its reason.
- [`src-tauri/src/sync.rs`](../../src-tauri/src/sync.rs) — the same division by
  accounts: a store that fails is one line in `failures`, not the end of the
  pass.
- [`src-tauri/src/prices.rs`](../../src-tauri/src/prices.rs) — the expensive
  operation is to identify each wished-for game, and that is written game by
  game; the next pass makes the prices of a batch that failed again and does not
  search for anybody again.
- [`src/App.tsx`](../../src/App.tsx) — the message that turns the reason into
  something that the user reads.

## 🔗 Related agreements

- [Every store connector has a switch of its own](../connectors/switch-per-connector.md)
  — the same idea between stores: what breaks, breaks alone.
- [A price is a cache of the data of another person, and it is replaced complete](../storage/prices-are-a-cache-that-is-replaced.md)
  — why a cancel in the middle of the prices deletes nothing that is still
  applicable.
