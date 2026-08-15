//! Caso de uso de sincronización: pedir a cada tienda lo que hay y volcarlo en
//! `store_entry`.
//!
//! No escribe en `game`, ni en `game_link`, ni en `user_state`. Esa disciplina
//! es la que hace que sincronizar sea una operación segura de repetir.
//!
//! Recibe sus colaboradores en lugar de sacarlos del estado global: así se
//! puede probar de extremo a extremo contra un servidor de mentira y una base
//! de datos de verdad, sin arrancar Tauri.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use domain::{
    AuthContext, EntryKind, StoreAccount, StoreConnector, StoreEntry, StoreId, StoreSession,
};
use secrets::SecretStore;
use serde::Serialize;
use storage::Database;
use storage::repositories::{
    ConnectorStateRepository, StoreAccountRepository, StoreEntryRepository,
};
use time::OffsetDateTime;

use crate::error::AppError;
use crate::state::{AppState, credential_key};

/// Qué está pasando durante una sincronización. La UI lo recibe por eventos en
/// lugar de esperar callada a que termine.
#[derive(Debug, Clone, Serialize)]
pub struct SyncProgress {
    pub store: String,
    pub stage: &'static str,
    pub done: usize,
    pub total: usize,
}

/// Recibe el progreso. Un trait en vez del `AppHandle` de Tauri para que el
/// caso de uso se pueda probar sin arrancar la aplicación.
pub trait ProgressSink: Send + Sync {
    fn report(&self, progress: SyncProgress);
    /// La sincronización se para en el siguiente punto seguro, nunca a mitad de
    /// una escritura.
    fn cancelled(&self) -> bool {
        false
    }
}

/// Para cuando a nadie le interesa el progreso: los tests, por ejemplo.
pub struct Silent;
impl ProgressSink for Silent {
    fn report(&self, _progress: SyncProgress) {}
}

#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub owned: usize,
    pub wishlist: usize,
    pub removed: u64,
    /// Cuentas que han fallado, con el motivo. Una tienda caída no puede
    /// impedir que las demás se sincronicen.
    pub failures: Vec<SyncFailure>,
    /// Stores that were left out because their connector is switched off. It is
    /// said out loud: a library that quietly stops growing looks like a bug.
    pub skipped: Vec<String>,
    /// El usuario paró a mitad. Lo ya volcado se queda: es idempotente.
    pub cancelled: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncFailure {
    pub store: String,
    pub account: String,
    pub reason: String,
}

pub async fn sync_all(
    state: &AppState,
    progress: &dyn ProgressSink,
) -> Result<SyncReport, AppError> {
    let secrets = state.secrets().await?;
    sync_stores(&state.db, secrets.as_ref(), &state.connectors, progress).await
}

/// Every connected account, one after another.
///
/// It takes its collaborators instead of the global state so that the switch
/// and the isolation between stores can be proved against a pretend server and
/// a real database, without starting Tauri.
pub async fn sync_stores(
    db: &Database,
    secrets: &dyn SecretStore,
    available: &HashMap<StoreId, Arc<dyn StoreConnector>>,
    progress: &dyn ProgressSink,
) -> Result<SyncReport, AppError> {
    let accounts = StoreAccountRepository(db).active().await?;
    let connectors = ConnectorStateRepository(db);
    let disabled = disabled_stores(&connectors).await?;
    let mut report = SyncReport::default();
    let total = accounts.len();

    for (index, account) in accounts.into_iter().enumerate() {
        if progress.cancelled() {
            report.cancelled = true;
            break;
        }
        // A switched off connector is not asked anything, not even for a token.
        // That is the whole point of the switch: a store whose authentication
        // broke stops costing the user a failed request on every run.
        if disabled.contains(&account.store) {
            let name = account.store.as_str().to_owned();
            if !report.skipped.contains(&name) {
                report.skipped.push(name);
            }
            continue;
        }
        progress.report(SyncProgress {
            store: account.store.as_str().to_owned(),
            stage: "biblioteca",
            done: index,
            total,
        });
        let result = match available.get(&account.store) {
            Some(connector) => {
                sync_account(db, secrets, connector.as_ref(), &account, &mut report).await
            }
            None => Err(AppError::Message(format!(
                "sin conector para {}",
                account.store.as_str()
            ))),
        };

        // The reason is written down and not only reported, because the next
        // time the user opens the application the report is gone and the empty
        // library is still there. With several accounts of one store the last
        // one has the say, which is the one the user just watched run.
        match result {
            Ok(()) => connectors.record_error(account.store, None).await?,
            Err(error) => {
                let reason = error.to_string();
                connectors
                    .record_error(account.store, Some(&reason))
                    .await?;
                report.failures.push(SyncFailure {
                    store: account.store.as_str().to_owned(),
                    account: account.display_name.unwrap_or(account.account_ref),
                    reason,
                });
            }
        }
    }

    Ok(report)
}

async fn disabled_stores(
    connectors: &ConnectorStateRepository<'_>,
) -> Result<HashSet<StoreId>, AppError> {
    Ok(connectors
        .all()
        .await?
        .into_iter()
        .filter(|state| !state.enabled)
        .map(|state| state.store)
        .collect())
}

pub async fn sync_account(
    db: &Database,
    secrets: &dyn SecretStore,
    connector: &dyn StoreConnector,
    account: &StoreAccount,
    report: &mut SyncReport,
) -> Result<(), AppError> {
    let key = credential_key(account);
    let credential = secrets.get(&key)?.ok_or(AppError::MissingCredential)?;

    let session = restore_session(connector, account, credential.clone()).await?;

    // Si el conector ha renovado la credencial hay que guardarla antes de nada.
    // GOG rota el token de refresco al usarlo: perder el nuevo deja la cuenta
    // sin forma de volver a entrar, y no se notaría hasta la próxima caducidad.
    if session.credential != credential {
        secrets.set(&key, &session.credential)?;
    }

    let entries = StoreEntryRepository(db);

    let owned = connector.owned(&session, account.id).await?;
    report.owned += owned.len();
    entries.upsert_many(&owned).await?;
    report.removed += entries
        .soft_delete_missing(account.id, EntryKind::Owned, &app_ids(&owned))
        .await?;

    let wishlist = connector.wishlist(&session, account.id).await?;
    report.wishlist += wishlist.len();
    entries.upsert_many(&wishlist).await?;
    report.removed += entries
        .soft_delete_missing(account.id, EntryKind::Wishlist, &app_ids(&wishlist))
        .await?;

    StoreAccountRepository(db)
        .mark_synced(account.id, OffsetDateTime::now_utc())
        .await?;
    Ok(())
}

/// Reconstruye la sesión a partir de lo guardado. El conector decide qué
/// significa su propia credencial; aquí solo se transporta.
async fn restore_session(
    connector: &dyn StoreConnector,
    account: &StoreAccount,
    credential: String,
) -> Result<StoreSession, AppError> {
    let mut session = connector
        .authenticate(&AuthContext::Stored { credential })
        .await?;
    session.account_ref = account.account_ref.clone();
    session.display_name = account.display_name.clone();
    Ok(session)
}

fn app_ids(entries: &[StoreEntry]) -> Vec<String> {
    entries.iter().map(|e| e.store_app_id.clone()).collect()
}
