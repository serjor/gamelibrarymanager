# 🎯 A price is a cache of the data of another person, and it is replaced complete

## 💡 Convention

Every table in this schema marks a row as gone with `deleted_at` and keeps it.
The two price tables are the only exception: their rows are deleted for real,
and a refresh replaces the whole set of prices of a game in one transaction.

The rule that forbids physical deletion protects what the user cannot get back.
A copy that left a store keeps its state, its notes and its history, and a device
that syncs later must not resurrect it. A price is the opposite of all that: it
belongs to a shop, it changes by the hour, and the next refresh brings it again.

So, three parts:

1. **Replace, never accumulate.** `save` deletes the rows of that game and
   writes the ones that just arrived. An offer that ended has to disappear,
   because an offer that is over still looks like an offer on screen.
2. **Forget what left the list.** A game that is no longer wished loses its
   prices. The list of what to keep is read **before** the pass, so stopping
   halfway never deletes something that is still valid.
3. **Never reach further than the price.** The refresh writes in
   `price_snapshot`, in `price_low` and in the two identifier columns of `game`.
   It does not touch `store_entry`, which belongs to the store, or `user_state`,
   which belongs to the user.

The whole set of price tables can be dropped and rebuilt with one refresh. If
that ever stops being true, something that is not a cache got in.

## 🏆 Benefits

- No ended offer survives on screen, which is the one failure that would make
  the whole screen untrustworthy.
- The exception is bounded and written down, so the next table nobody can decide
  about has a precedent to compare against instead of an argument to repeat.
- A refresh cancelled halfway leaves the library exactly as it was: the deletion
  depends on the wish list, not on how far the pass got.
- Dropping the two tables is a valid repair, and it costs one refresh.

## 👀 Examples

### ✅ Good

Replace inside the transaction that writes the new prices:

```rust
sqlx::query("DELETE FROM price_snapshot WHERE game_id = ?")
    .bind(&id)
    .execute(&mut *tx)
    .await?;

for deal in &prices.deals {
    // …insert every shop that sells it right now
}
```

Forget with the complete list, taken before the pass:

```rust
let live: Vec<GameId> = targets.iter().map(|target| target.game_id).collect();
prices.forget_missing(&live).await?;
```

### ❌ Bad

```rust
// Soft deleting a price. The offer ended two weeks ago and the row is still
// there, so every query has to remember to filter it, and the day one forgets
// the screen shows a discount that does not exist.
sqlx::query("UPDATE price_snapshot SET deleted_at = ? WHERE game_id = ?")
```

```rust
// Upsert without deleting first. GOG stops selling it, its row stays, and the
// cheapest price of the game is an offer nobody honours.
"INSERT INTO price_snapshot … ON CONFLICT (game_id, shop) DO UPDATE SET …"
```

```rust
// Forgetting with what was refreshed instead of with the whole wish list. A
// cancelled pass wipes the prices of everything it did not get to.
prices.forget_missing(&refreshed).await?;
```

```rust
// Writing the identifier of the price provider inside `Game`. Enriching a
// record with IGDB rewrites the whole row, so the identifier would be lost on
// every match and nobody would notice until the next price query.
game.itad_id = Some(found.id);
games.upsert(&game).await?;
```

## 🧐 Real world examples

- [`migrations/0007_prices.up.sql`](../../migrations/0007_prices.up.sql) — the
  two tables, and the reason the exception is bounded to them.
- [`crates/storage/src/repositories/price.rs`](../../crates/storage/src/repositories/price.rs)
  — `save` replaces, `forget_missing` takes what to keep and not what to delete.
- [`crates/storage/src/repositories/game.rs`](../../crates/storage/src/repositories/game.rs)
  — `set_itad` is its own statement, out of reach of `upsert`.
- [`crates/storage/tests/prices.rs`](../../crates/storage/tests/prices.rs) — an
  offer that ended disappears, and neither the copy of the store nor the state
  of the user moves.
- [`src-tauri/tests/prices.rs`](../../src-tauri/tests/prices.rs) — buying a
  wished game removes its price on the next pass.

## 🔗 Related agreements

- [To add metadata to a record writes its row again; it does not make a new one](enrich-records-in-place.md)
  — the second half of what protects `user_state`: the record keeps its
  identifier, and everything attached to it survives.
- [Money is kept in whole cents](../domain/money-in-cents.md) — what goes inside
  these rows.
- [Every store connector has a switch of its own](../connectors/switch-per-connector.md)
  — the same reasoning applied to a provider that breaks: prices have their own
  button and their own failure, and a synchronisation never waits for them.
