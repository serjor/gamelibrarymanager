//! The shared state of the application: the database, the store of secrets and
//! the available connectors.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use connectors::{EpicConnector, GogConnector, SteamConnector};
use domain::{StoreAccount, StoreConnector, StoreId};
use metadata::{IgdbClient, ItadClient};
use secrets::{Backend, EncryptedFileStore, KeyringStore, SecretStore};
use std::path::PathBuf;
use storage::Database;
use tokio::sync::RwLock;

pub const SERVICE: &str = "com.serjor.gamelibrarymanager";

/// The keys under which the secrets that do not belong to a store account live.
/// IGDB prohibits a secret inside the binary, thus these secrets belong to the
/// user and live where all of the other secrets live: in the store, never in
/// SQLite.
pub const IGDB_CREDENTIALS: &str = "igdb:credentials";
pub const IGDB_TOKEN: &str = "igdb:token";
/// The ITAD key and the country with which the prices are requested. The country
/// goes with the key because without it the prices are those of a different
/// market.
pub const ITAD_CREDENTIALS: &str = "itad:credentials";

pub struct AppState {
    pub db: Database,
    pub igdb: IgdbClient,
    pub itad: ItadClient,
    pub connectors: HashMap<StoreId, Arc<dyn StoreConnector>>,
    /// The store can not yet exist: with no keyring, the user must write a
    /// passphrase before the application can keep anything.
    secrets: RwLock<Option<Arc<dyn SecretStore>>>,
    pub backend: Backend,
    secrets_path: PathBuf,
    /// The cancel flag of the long operation in progress. There is only one
    /// because a synchronisation and a match never run at the same time: each of
    /// the two buttons disables the other while one operation runs.
    cancel_flag: AtomicBool,
}

impl AppState {
    pub fn new(db: Database, secrets_path: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("gamelibrarymanager/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();

        let http_for_igdb = http.clone();
        let http_for_itad = http.clone();
        let mut connectors: HashMap<StoreId, Arc<dyn StoreConnector>> = HashMap::new();
        connectors.insert(StoreId::Steam, Arc::new(SteamConnector::new(http.clone())));
        connectors.insert(StoreId::Gog, Arc::new(GogConnector::new(http.clone())));
        // Registered like any other. What sets Epic apart is not how it is
        // built but that it can be switched off: `connector_state` says so and
        // the synchronisation obeys.
        connectors.insert(StoreId::Epic, Arc::new(EpicConnector::new(http)));

        let backend = secrets::detect(SERVICE);
        let secrets: Option<Arc<dyn SecretStore>> = match backend {
            Backend::Keyring => Some(Arc::new(KeyringStore::new(SERVICE))),
            Backend::Passphrase => None,
        };

        Self {
            db,
            igdb: IgdbClient::new(http_for_igdb),
            itad: ItadClient::new(http_for_itad),
            connectors,
            secrets: RwLock::new(secrets),
            backend,
            secrets_path,
            cancel_flag: AtomicBool::new(false),
        }
    }

    /// Opens the encrypted store. It is necessary only when there is no
    /// keyring.
    pub async fn unlock(&self, passphrase: &str) -> Result<(), secrets::SecretsError> {
        let store = EncryptedFileStore::open(&self.secrets_path, passphrase)?;
        *self.secrets.write().await = Some(Arc::new(store));
        Ok(())
    }

    pub async fn secrets(&self) -> Result<Arc<dyn SecretStore>, secrets::SecretsError> {
        self.secrets
            .read()
            .await
            .clone()
            .ok_or(secrets::SecretsError::Unavailable)
    }

    pub fn begin_operation(&self) {
        self.cancel_flag.store(false, Ordering::Relaxed);
    }

    pub fn cancel_operation(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub fn end_operation(&self) {
        self.cancel_flag.store(false, Ordering::Relaxed);
    }

    pub fn operation_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    pub async fn is_unlocked(&self) -> bool {
        self.secrets.read().await.is_some()
    }
}

/// The name under which the credential of an account is kept. It is never kept
/// in SQLite: the database knows that the account exists, not how to open it.
pub fn credential_key(account: &StoreAccount) -> String {
    format!("{}:{}", account.store.as_str(), account.account_ref)
}
