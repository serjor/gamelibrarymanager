//! Estado compartido de la aplicación: la base de datos, el almacén de secretos
//! y los conectores disponibles.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use connectors::SteamConnector;
use domain::{StoreAccount, StoreConnector, StoreId};
use metadata::IgdbClient;
use secrets::{Backend, EncryptedFileStore, KeyringStore, SecretStore};
use std::path::PathBuf;
use storage::Database;
use tokio::sync::RwLock;

pub const SERVICE: &str = "com.serjor.gamelibrarymanager";

/// Claves bajo las que viven los secretos que no pertenecen a una cuenta de
/// tienda. IGDB prohíbe empotrar el secreto en el binario, así que son del
/// usuario y viven donde vive todo lo demás: en el almacén, nunca en SQLite.
pub const IGDB_CREDENTIALS: &str = "igdb:credentials";
pub const IGDB_TOKEN: &str = "igdb:token";

pub struct AppState {
    pub db: Database,
    pub igdb: IgdbClient,
    pub connectors: HashMap<StoreId, Arc<dyn StoreConnector>>,
    /// El almacén puede no existir todavía: sin keyring hace falta que el
    /// usuario escriba una contraseña antes de poder guardar nada.
    secrets: RwLock<Option<Arc<dyn SecretStore>>>,
    pub backend: Backend,
    secrets_path: PathBuf,
    /// Bandera de cancelación de la sincronización en curso.
    cancel_sync: AtomicBool,
}

impl AppState {
    pub fn new(db: Database, secrets_path: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("gamelibrarymanager/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();

        let http_for_igdb = http.clone();
        let mut connectors: HashMap<StoreId, Arc<dyn StoreConnector>> = HashMap::new();
        connectors.insert(StoreId::Steam, Arc::new(SteamConnector::new(http)));

        let backend = secrets::detect(SERVICE);
        let secrets: Option<Arc<dyn SecretStore>> = match backend {
            Backend::Keyring => Some(Arc::new(KeyringStore::new(SERVICE))),
            Backend::Passphrase => None,
        };

        Self {
            db,
            igdb: IgdbClient::new(http_for_igdb),
            connectors,
            secrets: RwLock::new(secrets),
            backend,
            secrets_path,
            cancel_sync: AtomicBool::new(false),
        }
    }

    /// Abre el almacén cifrado. Solo hace falta cuando no hay keyring.
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

    pub fn begin_sync(&self) {
        self.cancel_sync.store(false, Ordering::Relaxed);
    }

    pub fn cancel_sync(&self) {
        self.cancel_sync.store(true, Ordering::Relaxed);
    }

    pub fn end_sync(&self) {
        self.cancel_sync.store(false, Ordering::Relaxed);
    }

    pub fn sync_cancelled(&self) -> bool {
        self.cancel_sync.load(Ordering::Relaxed)
    }

    pub async fn is_unlocked(&self) -> bool {
        self.secrets.read().await.is_some()
    }
}

/// Nombre bajo el que se guarda la credencial de una cuenta. Nunca se guarda en
/// SQLite: la base de datos sabe que la cuenta existe, no cómo entrar en ella.
pub fn credential_key(account: &StoreAccount) -> String {
    format!("{}:{}", account.store.as_str(), account.account_ref)
}
