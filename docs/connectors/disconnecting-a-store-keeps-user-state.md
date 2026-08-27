# 🎯 Disconnecting a store keeps what the user wrote

## 💡 Convention

Disconnecting an account is a logical delete. It marks the account and its
copies as gone, and removes the credential from the secret store.

It does not delete the game, the link, or `user_state`. The store gave those
copies to the application, and the user may have added a status, a score, or
notes to the resulting record. A new connection can show the copies again
without losing that history.

The operation uses the store and the account reference. The database finds the
account, so the interface never receives or stores a credential.

## 🏆 Benefits

- A user can stop using one store without losing the library that it built.
- The next synchronisation ignores the disconnected account.
- Removing the credential reduces access to the store at the same time.
- Connecting the account again can restore its copies because the history stays.

## 👀 Examples

### ✅ Good

Mark the account and its copies in one transaction. Then remove the credential
with the same account identity:

```rust
StoreAccountRepository(db).soft_delete(account.id).await?;
secrets.delete(&credential_key(&account))?;
```

The synchronisation reads active accounts. A disconnected account is not in this
list, so its connector is not called:

```rust
let accounts = StoreAccountRepository(db).active().await?;
```

### ❌ Bad

```rust
// This loses the record that the user may have rated or annotated.
sqlx::query("DELETE FROM store_account WHERE id = ?")
    .bind(account_id)
    .execute(db.pool())
    .await?;
```

```rust
// This leaves access to the store after the user asked to disconnect it.
StoreAccountRepository(db).soft_delete(account.id).await?;
// The credential remains in the secret store.
```

```rust
// The account is still active, so the next pass can ask the store again.
let account = account_from_the_interface;
connector.owned(&session, account.id).await?;
```

## 🧐 Real world examples

- [`crates/storage/src/repositories/store_account.rs`](../../crates/storage/src/repositories/store_account.rs)
  marks the account and its copies in one transaction.
- [`src-tauri/src/commands/mod.rs`](../../src-tauri/src/commands/mod.rs)
  finds the account by store and reference, then removes its credential.
- [`src-tauri/src/sync.rs`](../../src-tauri/src/sync.rs) reads only active
  accounts before it calls a connector.
- [`src-tauri/tests/disconnect.rs`](../../src-tauri/tests/disconnect.rs) checks
  the account, credential, copy, link, user state, and next synchronisation.
- [`src/App.tsx`](../../src/App.tsx) asks for confirmation and says that the
  records and notes stay.

## 🔗 Related agreements

- [No store credential goes inside the binary](credentials-outside-the-binary.md)
  — the credential belongs only in the secret store.
- [Every store connector has a switch of its own](switch-per-connector.md)
  — disabling a connector also keeps the data that it gave.
- [A record with no live copy stays visible](../ui/record-with-no-live-copy.md)
  — a disconnected copy leaves a record that the user can still find.
