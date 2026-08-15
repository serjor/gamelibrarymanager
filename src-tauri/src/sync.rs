//! The synchronisation use case: to ask each store what is there and write it in
//! `store_entry`.
//!
//! It does not write in `game`, in `game_link` or in `user_state`. That
//! discipline is what makes a synchronisation an operation that is safe to
//! repeat.
//!
//! It takes its collaborators and does not get them from the global state: thus
//! you can test it from end to end against a pretend server and a real database,
//! with no start of Tauri.

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

/// What occurs during a synchronisation. The interface receives it through
/// events and does not wait quietly for the end.
#[derive(Debug, Clone, Serialize)]
pub struct SyncProgress {
    pub store: String,
    pub stage: &'static str,
    pub done: usize,
    pub total: usize,
}

/// Receives the progress. A trait and not the `AppHandle` of Tauri, so that you
/// can test the use case without you start the application.
pub trait ProgressSink: Send + Sync {
    fn report(&self, progress: SyncProgress);
    /// The synchronisation stops at the next safe point, never in the middle of
    /// a write.
    fn cancelled(&self) -> bool {
        false
    }
}

/// For when nobody wants the progress: the tests, for example.
pub struct Silent;
impl ProgressSink for Silent {
    fn report(&self, _progress: SyncProgress) {}
}

#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub owned: usize,
    pub wishlist: usize,
    pub removed: u64,
    /// The accounts that failed, with the reason. A store that is down cannot
    /// prevent the synchronisation of the other stores.
    pub failures: Vec<SyncFailure>,
    /// Stores that were left out because their connector is switched off. It is
    /// said out loud: a library that quietly stops growing looks like a bug.
    pub skipped: Vec<String>,
    /// The user stopped in the middle. The data already written stays: the
    /// operation is idempotent.
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
            stage: "library",
            done: index,
            total,
        });
        let result = match available.get(&account.store) {
            Some(connector) => {
                sync_account(db, secrets, connector.as_ref(), &account, &mut report).await
            }
            None => Err(AppError::Message(format!(
                "there is no connector for {}",
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

    // If the connector renewed the credential, you must keep it before anything
    // else. GOG changes the refresh token when you use it: to lose the new token
    // leaves the account with no way back in, and nobody would see that until
    // the next expiry.
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

/// Builds the session again from the data kept. The connector decides what its
/// own credential means; this code only carries it.
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
