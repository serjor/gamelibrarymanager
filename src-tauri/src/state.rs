//! The shared state of the application: the database, the store of secrets and
//! the available connectors.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use connectors::{EpicConnector, GogConnector, SteamConnector};
use domain::{StoreAccount, StoreConnector, StoreId};
use metadata::{IgdbClient, ItadClient};
use secrets::{Backend, EncryptedFileStore, KeyringStore, SecretStore};
use std::path::PathBuf;
use storage::Database;
use tokio::sync::RwLock;

pub const SERVICE: &str = "com.serjor.gamelibrarymanager";

/// The time that the application waits for a provider before it stops.
///
/// `reqwest` puts no limit of its own. A store that accepts the connection and
/// then says nothing holds the synchronisation for ever, and the cancel flag
/// does not reach it: the synchronisation reads that flag between accounts,
/// never inside a request. Thirty seconds is much more than the five providers
/// need — they answer in less than one — and it is a time that a person can
/// wait.
///
/// The login windows of GOG and of Epic do not use this client. They wait on a
/// channel, and this limit does not apply to them.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The part of the limit that belongs to the connection alone. A host that does
/// not answer at all must fail before the complete time.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The client that the three connectors, IGDB and ITAD share.
pub fn http_client() -> reqwest::Client {
    http_client_with(REQUEST_TIMEOUT)
}

/// The same client with a different limit. The tests use it: a test that waits
/// thirty seconds is a test that nobody runs.
///
/// There is one builder and not two, thus a test cannot prove a limit that the
/// application does not apply.
pub fn http_client_with(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("gamelibrarymanager/", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        // Before, this line gave `Client::default()` when the builder failed.
        // That fallback did nothing: `default()` builds the same client with no
        // limit, and it panics in the same conditions. And a client with no
        // limit is exactly what this constant exists to prevent, thus the
        // failure is now said out loud.
        .expect("the HTTP client must build: without it there is no provider")
}

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
        let http = http_client();

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
