# 🎯 A database backup uses `VACUUM INTO`

## 💡 Convention

When a file database has migrations to apply, the application makes a complete
copy before it runs them. It uses SQLite's `VACUUM INTO` statement through the
open connection.

The database uses WAL. The main file can then be missing committed pages that
are still in the WAL file. A file copy does not know this. SQLite does know it,
so the vacuum reads the current database and writes a complete new file.

The copy has the database name and the migration version in its name. The
application keeps the three newest copies. A failed migration can therefore be
tried again without losing the previous copy.

## 🏆 Benefits

- A migration has a recoverable copy before it changes the schema.
- The copy includes data from the main file and the WAL file.
- A failed or repeated migration does not create an unlimited list of files.
- The normal in-memory test database does not create files beside the project.

## 👀 Examples

### ✅ Good

Ask SQLite to make the copy while the configured connection is open:

```rust
let sql = format!("VACUUM INTO '{}'", sqlite_string(&backup));
sqlx::query(&sql).execute(self.pool()).await?;
```

Make the copy only when an up migration is pending:

```rust
if let Some(path) = backup_path {
    db.backup_before_migrations(path).await?;
}
db.migrate().await?;
```

### ❌ Bad

```rust
// With WAL, this can omit committed data that is not in library.db yet.
std::fs::copy("library.db", "library.db.bak")?;
```

```rust
// The new schema can fail and leave no copy of the old one.
db.migrate().await?;
```

```rust
// Every failed start leaves another file forever.
let backup = path.with_file_name(format!("library.db.bak-{}", random_id()));
```

## 🧐 Real world examples

- [`crates/storage/src/lib.rs`](../../crates/storage/src/lib.rs) checks pending
  migrations, uses `VACUUM INTO`, and keeps three copies.
- [`crates/storage/tests/backup.rs`](../../crates/storage/tests/backup.rs)
  opens an old schema, checks the copy, and checks the three-copy limit.
- [`migrations/`](../../migrations/) contains the up and down files that the
  database applies after the copy.

## 🔗 Related agreements

- [A price is a cache of the data of another person, and it is replaced complete](prices-are-a-cache-that-is-replaced.md)
  — the data that can be rebuilt differs from the database copy that protects
  user data.
- [To add metadata to a record writes its row again; it does not make a new one](enrich-records-in-place.md)
  — migration and enrichment both keep the data that the user already owns.
