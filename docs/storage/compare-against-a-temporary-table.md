# 🎯 A delete that compares against a list of the provider uses a temporary table

## 💡 Convention

A synchronisation asks a store what the user has, and then it must mark what the
store no longer shows. The list that the store gives has thousands of
identifiers.

Put that list in a **temporary table**, inside the same transaction, and compare
against the table:

```sql
CREATE TEMP TABLE seen (app_id TEXT PRIMARY KEY);
-- the identifiers go in, in batches
UPDATE store_entry SET deleted_at = ?, updated_at = ?
 WHERE account_id = ? AND kind = ? AND deleted_at IS NULL
   AND store_app_id NOT IN (SELECT app_id FROM seen);
DROP TABLE seen;
```

Never a `NOT IN` with one placeholder for each identifier. SQLite accepts a
limited number of parameters in one statement, and a usual library passes it.

And **never divide the comparison into batches**. It is the answer that looks
correct when the statement gets too large, and it deletes the library: with the
identifiers divided in two, the first `NOT IN` marks everything that is in the
second half, and the second marks everything that is in the first. The batches
divide the **writing** of the list into the table, which changes no result.

Two more rules that come with it:

- Drop the table before you create it. The pool gives one connection and it is
  used again, thus a table that a failure before left behind would still be
  there with its rows.
- An empty list is a correct list. A store that gives nothing means a library
  with nothing, and every copy of that account gets the logical delete.

## 🏆 Benefits

- A library of five thousand copies works the same as one of five.
- The delete stays one statement over one set, which is what makes it correct.
  The batches are only a way to write, and they cannot change what is deleted.
- The whole operation is one transaction: a failure in the middle leaves the
  library exactly as it was.
- The comparison uses an index of the temporary table instead of a list of
  thousands of literal values.

## 👀 Examples

### ✅ Good

```rust
sqlx::query("DROP TABLE IF EXISTS temp.seen").execute(&mut *tx).await?;
sqlx::query("CREATE TEMP TABLE seen (app_id TEXT PRIMARY KEY)").execute(&mut *tx).await?;

for batch in seen.chunks(Self::SEEN_BATCH) {
    let values = vec!["(?)"; batch.len()].join(",");
    let sql = format!("INSERT OR IGNORE INTO seen (app_id) VALUES {values}");
    let mut insert = sqlx::query(&sql);
    for app_id in batch {
        insert = insert.bind(app_id);
    }
    insert.execute(&mut *tx).await?;
}
```

### ❌ Bad

```rust
// One placeholder for each identifier. Two thousand copies, two thousand
// parameters, and the statement stops being accepted.
let placeholders = vec!["?"; seen.len()].join(",");
let sql = format!("… AND store_app_id NOT IN ({placeholders})");
```

```rust
// The comparison divided into batches. The first pass marks everything that is
// in the second batch, and the second pass marks the rest: the account loses
// all of its copies.
for batch in seen.chunks(500) {
    soft_delete_missing(account_id, kind, batch).await?;
}
```

```rust
// The temporary table with no transaction and with no `DROP`. The next
// synchronisation finds the identifiers of the one before, and it deletes
// nothing.
sqlx::query("CREATE TEMP TABLE seen (app_id TEXT PRIMARY KEY)").execute(pool).await?;
```

## 🧐 Real world examples

- [`crates/storage/src/repositories/store_entry.rs`](../../crates/storage/src/repositories/store_entry.rs)
  — `soft_delete_missing`, with `SEEN_BATCH` and the reason beside it.
- [`crates/storage/tests/store_entry.rs`](../../crates/storage/tests/store_entry.rs)
  — two thousand copies where fifteen hundred stay: exactly five hundred get the
  logical delete. With the comparison divided into batches this test fails with
  two thousand.

## 🔗 Related agreements

- [To add metadata to a record writes its row again; it does not make a new one](enrich-records-in-place.md)
  — why the delete is logical: the copy can come back, and what the user wrote
  is still attached to it.
- [A long pass saves as it goes, and a provider that cuts it off is a result](../tauri/long-passes-save-in-batches.md)
  — the other place where a batch appears, and there it does change what is
  written: it is a save point, not a division of a comparison.
