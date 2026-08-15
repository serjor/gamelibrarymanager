# 🎯 No store credential goes inside the binary

## 💡 Convention

No key, no secret and no token goes inside the binary or the repository. The user
supplies **all** of the credentials when they connect the account. The
credentials live in the store of secrets
([`crates/secrets`](../../crates/secrets/src/lib.rs)): the native keyring of the
operating system, or a file encrypted with a passphrase if the session has no
secret-service.

Never in SQLite, never in a configuration file, never in a log.

The rule applies also when the secret is public. GOG does not let you register
third-party applications. The only client that its authorisation server accepts
is the client of GOG Galaxy, and its `client_id`/`client_secret` pair has been
published in gogdl for years. The program still does **not** put it inside. It
asks the user for the pair, it puts the pair in the store with the tokens, and
the screen says clearly that the pair does not belong to the user and that it is
the same for all users. To call it "your key" would be false; to put it inside
the binary would break the rule for convenience.

The program also **never** asks for the user name and the password of a store.
Steam uses an API key of the user. GOG uses its own login page in a webview, and
the program takes only the `code` of the redirect.

There is a practical result: the credential that the connector gives is an
**opaque** block (`StoreSession::credential`). The remainder of the system moves
it between the store and the connector and does not read it. It carries what
that connector needs to renew itself — the client credentials included.

## 🏆 Benefits

- A binary with no secret gives nothing away when you distribute it, and you can
  read all of the repository and find no key.
- The developer agreement of IGDB **prohibits** a client secret inside a desktop
  application. To obey it with a general rule costs less than an exception for
  each provider.
- The Steam key of the user is also what opens the private library without the
  user makes their profile public. To ask for it is not only an obligation: it is
  what makes the product possible.
- With no shared credential there is no shared quota to use up and no single IP
  block that stops all of the users at the same time.
- To keep the client credentials **inside** the opaque credential makes the
  refresh operate alone, with no new question to the user at each expiry.

## 👀 Examples

### ✅ Good

```rust
/// The *client* credentials of a store: they identify the application, not the
/// user. The user supplies them when they connect the account, as with the
/// Steam key. GOG does not let you register a client of your own, thus the only
/// way to keep a secret out of the binary is to let the pair come in through the
/// same door as the other keys and live in the store.
pub struct ClientCredentials {
    pub client_id: String,
    pub client_secret: String,
}
```

```rust
// The credential is opaque and carries what it needs to renew itself.
struct GogCredential {
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    user_id: String,
    expires_at: i64,
}
```

```rust
// The credential goes to the store. The database only knows that the account
// exists, not how to open it.
state
    .secrets()
    .await?
    .set(&credential_key(&account), &session.credential)?;
```

### ❌ Bad

```rust
// Inside the binary: published as soon as a person looks at the executable, and
// against the developer agreement in the case of IGDB.
const GOG_CLIENT_SECRET: &str = "9d85c43b1482497dbbce61f6e4aa173a4337…";
```

```rust
// In SQLite: not encrypted, and it survives each backup copy of the file.
sqlx::query("UPDATE store_account SET api_key = ? WHERE id = ?")
```

```tsx
// To ask for the password of the store. The terms of use of Steam prohibit it
// clearly, and for GOG and Epic it is unnecessary.
<label>GOG password</label>
```

```tsx
// To say something false about what the screen asks for. The Galaxy pair does
// not belong to the user and it is identical for all users: to say so is part of
// the convention.
<p>Write your private GOG key</p>
```

## 🧐 Real world examples

- [`crates/domain/src/ports.rs`](../../crates/domain/src/ports.rs) —
  `AuthContext` and `ClientCredentials`: the contract that makes the pair come
  from outside.
- [`crates/connectors/src/gog/mod.rs`](../../crates/connectors/src/gog/mod.rs) —
  `GogCredential` and the exchange of the code; none of this is kept in the
  binary.
- [`src-tauri/src/commands/gog.rs`](../../src-tauri/src/commands/gog.rs) — it
  opens the real GOG page and sees only the `code`; the password does not come
  through the process.
- [`src/features/onboarding/GogSetup.tsx`](../../src/features/onboarding/GogSetup.tsx)
  — the screen that says clearly that the pair does not belong to the user.
- [`crates/secrets/src/lib.rs`](../../crates/secrets/src/lib.rs) — `SecretStore`
  and `detect`, which really examines whether the keyring answers and does not
  assume the answer from the platform.
- [`src-tauri/tests/sync.rs`](../../src-tauri/tests/sync.rs) — it reads the bytes
  of the SQLite file to examine that the API key does not appear in the database.
- [`NOTICE`](../../NOTICE) — it records where the Galaxy pair comes from and says
  clearly that the program does not carry it inside.

## 🔗 Related agreements

- [Verify the unofficial endpoints before you write the connector](verify-unofficial-endpoints.md)
  — the second half of what you need to add a store.
- [Each interface link needs explicit scope in the capability](../tauri/url-scope-in-capabilities.md)
  — minimum privilege applied to the permissions of the window.
