//! The store of secrets. The API keys and the store tokens go here, and only
//! here: never to SQLite, never to a configuration file, never to a log.
//!
//! No `tauri-plugin-stronghold`: it goes away in Tauri v3.

mod encrypted_file;
mod keyring_store;

pub use encrypted_file::EncryptedFileStore;
pub use keyring_store::KeyringStore;

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("there is no store of secrets available on this system")]
    Unavailable,
    #[error("the passphrase does not open the store")]
    WrongPassphrase,
    #[error("the store is corrupt: {0}")]
    Corrupt(String),
    #[error("error of the system store: {0}")]
    Backend(String),
    #[error("input/output error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SecretsError>;

/// Keeps secrets and gets them back by name. There is deliberately no `list`:
/// nobody needs a list of the credentials, and the absence of the operation
/// removes the temptation.
pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// Which store you can use on this machine.
///
/// On Linux with no secret-service — containers, minimal desktops, remote
/// sessions — the native keyring does not exist, and to find that when you keep
/// the first key is the worst moment to find it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// The keyring of the operating system: there is no passphrase to remember.
    Keyring,
    /// An encrypted file: it asks the user for a passphrase.
    Passphrase,
}

/// Examines whether the keyring really answers, with a secret that it then
/// deletes. A question about the platform is not sufficient: two equal Linux
/// systems operate differently, and the difference is what started in the
/// session.
pub fn detect(service: &str) -> Backend {
    let probe = KeyringStore::new(service);
    let key = "__probe__";
    match probe.set(key, "ok").and_then(|()| probe.get(key)) {
        Ok(Some(value)) if value == "ok" => {
            let _ = probe.delete(key);
            Backend::Keyring
        }
        _ => Backend::Passphrase,
    }
}
