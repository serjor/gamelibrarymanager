//! Caso de uso de sincronización: pedir a cada tienda lo que hay y volcarlo en
//! `store_entry`.
//!
//! No escribe en `game`, ni en `game_link`, ni en `user_state`. Esa disciplina
//! es la que hace que sincronizar sea una operación segura de repetir.
//!
//! Recibe sus colaboradores en lugar de sacarlos del estado global: así se
//! puede probar de extremo a extremo contra un servidor de mentira y una base
//! de datos de verdad, sin arrancar Tauri.

use domain::{AuthContext, EntryKind, StoreAccount, StoreConnector, StoreEntry, StoreSession};
use secrets::SecretStore;
use serde::Serialize;
use storage::Database;
use storage::repositories::{StoreAccountRepository, StoreEntryRepository};
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
    let accounts = StoreAccountRepository(&state.db).active().await?;
    let secrets = state.secrets().await?;
    let mut report = SyncReport::default();
    let total = accounts.len();

    for (index, account) in accounts.into_iter().enumerate() {
        if progress.cancelled() {
            report.cancelled = true;
            break;
        }
        progress.report(SyncProgress {
            store: account.store.as_str().to_owned(),
            stage: "biblioteca",
            done: index,
            total,
        });
        let result = match state.connectors.get(&account.store) {
            Some(connector) => {
                sync_account(
                    &state.db,
                    secrets.as_ref(),
                    connector.as_ref(),
                    &account,
                    &mut report,
                )
                .await
            }
            None => Err(AppError::Message(format!(
                "sin conector para {}",
                account.store.as_str()
            ))),
        };

        if let Err(error) = result {
            report.failures.push(SyncFailure {
                store: account.store.as_str().to_owned(),
                account: account.display_name.unwrap_or(account.account_ref),
                reason: error.to_string(),
            });
        }
    }

    Ok(report)
}

pub async fn sync_account(
    db: &Database,
    secrets: &dyn SecretStore,
    connector: &dyn StoreConnector,
    account: &StoreAccount,
    report: &mut SyncReport,
) -> Result<(), AppError> {
    let credential = secrets
        .get(&credential_key(account))?
        .ok_or(AppError::MissingCredential)?;

    let session = restore_session(connector, account, credential).await?;
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
