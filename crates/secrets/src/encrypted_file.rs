//! The alternative for when there is no keyring: a file encrypted with a key
//! derived from the passphrase of the user.
//!
//! Argon2id derives the key and XChaCha20-Poly1305 encrypts. The passphrase is
//! kept in no place: if the user loses it, the application asks for the API keys
//! again. That is an acceptable nuisance and it is better than a store that a
//! different person can open.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng, rand_core::RngCore};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

use crate::{Result, SecretStore, SecretsError};

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;

pub struct EncryptedFileStore {
    path: PathBuf,
    key: Key,
    salt: [u8; SALT_LEN],
}

impl EncryptedFileStore {
    /// Opens the store, or creates it if it does not yet exist. An incorrect
    /// passphrase is found here and not when the first secret is read: the
    /// encryption is authenticated.
    pub fn open(path: &Path, passphrase: &str) -> Result<Self> {
        let (salt, existing) = match fs::read(path) {
            Ok(bytes) if bytes.len() >= SALT_LEN => {
                let mut salt = [0u8; SALT_LEN];
                salt.copy_from_slice(&bytes[..SALT_LEN]);
                (salt, Some(bytes[SALT_LEN..].to_vec()))
            }
            Ok(_) => return Err(SecretsError::Corrupt("truncated header".to_owned())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut salt = [0u8; SALT_LEN];
                OsRng.fill_bytes(&mut salt);
                (salt, None)
            }
            Err(e) => return Err(SecretsError::Io(e)),
        };

        let key = derive_key(passphrase, &salt)?;
        let store = Self {
            path: path.to_owned(),
            key,
            salt,
        };

        match existing {
            // To decrypt the data that was there is the test of the
            // passphrase.
            Some(payload) if !payload.is_empty() => {
                store.decrypt(&payload)?;
            }
            _ => store.write(&BTreeMap::new())?,
        }

        Ok(store)
    }

    fn read(&self) -> Result<BTreeMap<String, String>> {
        match fs::read(&self.path) {
            Ok(bytes) if bytes.len() > SALT_LEN => self.decrypt(&bytes[SALT_LEN..]),
            Ok(_) => Ok(BTreeMap::new()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(SecretsError::Io(e)),
        }
    }

    fn write(&self, secrets: &BTreeMap<String, String>) -> Result<()> {
        let plaintext =
            serde_json::to_vec(secrets).map_err(|e| SecretsError::Corrupt(e.to_string()))?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = XChaCha20Poly1305::new(&self.key)
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|_| SecretsError::Corrupt("could not encrypt".to_owned()))?;

        let mut bytes = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
        bytes.extend_from_slice(&self.salt);
        bytes.extend_from_slice(&nonce_bytes);
        bytes.extend_from_slice(&ciphertext);

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, bytes)?;
        Ok(())
    }

    fn decrypt(&self, payload: &[u8]) -> Result<BTreeMap<String, String>> {
        if payload.len() <= NONCE_LEN {
            return Err(SecretsError::Corrupt("truncated payload".to_owned()));
        }
        let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
        let plaintext = XChaCha20Poly1305::new(&self.key)
            .decrypt(XNonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| SecretsError::WrongPassphrase)?;
        serde_json::from_slice(&plaintext).map_err(|e| SecretsError::Corrupt(e.to_string()))
    }
}

/// `Debug` written by hand and with the key removed: a derived `Debug` would
/// show the encryption key at the first `dbg!` or at the first error that
/// carries the store.
impl std::fmt::Debug for EncryptedFileStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptedFileStore")
            .field("path", &self.path)
            .field("key", &"<removed>")
            .finish()
    }
}

impl SecretStore for EncryptedFileStore {
    fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut secrets = self.read()?;
        secrets.insert(key.to_owned(), value.to_owned());
        self.write(&secrets)
    }

    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.read()?.get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<()> {
        let mut secrets = self.read()?;
        secrets.remove(key);
        self.write(&secrets)
    }
}

fn derive_key(passphrase: &str, salt: &[u8; SALT_LEN]) -> Result<Key> {
    let mut bytes = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut bytes)
        .map_err(|e| SecretsError::Corrupt(e.to_string()))?;
    Ok(*Key::from_slice(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("secrets.bin");
        (dir, path)
    }

    #[test]
    fn keeps_and_gets_back() {
        let (_dir, path) = tmp();
        let store = EncryptedFileStore::open(&path, "a long passphrase").expect("open");
        store.set("steam:api_key", "ABC123").expect("keep");
        assert_eq!(
            store.get("steam:api_key").expect("read"),
            Some("ABC123".to_owned())
        );
        store.delete("steam:api_key").expect("delete");
        assert_eq!(store.get("steam:api_key").expect("read"), None);
    }

    #[test]
    fn the_secret_is_not_readable_in_the_file() {
        let (_dir, path) = tmp();
        let store = EncryptedFileStore::open(&path, "a long passphrase").expect("open");
        store
            .set("steam:api_key", "SECRET_IN_THE_CLEAR")
            .expect("keep");

        let bytes = fs::read(&path).expect("read the file");
        assert!(
            !bytes.windows(19).any(|w| w == b"SECRET_IN_THE_CLEAR"),
            "the file cannot contain the readable secret"
        );
    }

    #[test]
    fn an_incorrect_passphrase_does_not_open_the_store() {
        let (_dir, path) = tmp();
        EncryptedFileStore::open(&path, "the correct one")
            .expect("open")
            .set("k", "v")
            .expect("keep");

        let error =
            EncryptedFileStore::open(&path, "the incorrect one").expect_err("it must not open");
        assert!(matches!(error, SecretsError::WrongPassphrase));
    }

    #[test]
    fn it_survives_a_close_and_a_new_open() {
        let (_dir, path) = tmp();
        EncryptedFileStore::open(&path, "key")
            .expect("open")
            .set("gog:token", "refresh-token")
            .expect("keep");

        let reopened = EncryptedFileStore::open(&path, "key").expect("open again");
        assert_eq!(
            reopened.get("gog:token").expect("read"),
            Some("refresh-token".to_owned())
        );
    }
}
