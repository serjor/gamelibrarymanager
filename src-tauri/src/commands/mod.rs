//! Comandos expuestos a la UI. Orquestan casos de uso y traducen tipos: la
//! lógica vive en los crates de dominio y adaptadores.

use domain::{AuthContext, EntryKind, StoreAccount, StoreAccountId, StoreId};
use serde::Serialize;
use storage::repositories::{StoreAccountRepository, StoreEntryRepository};
use tauri::State;
use time::OffsetDateTime;

use crate::error::AppError;
use crate::state::{AppState, credential_key};
use crate::sync::{self, SyncReport};

#[derive(Serialize)]
pub struct AppInfo {
    pub version: &'static str,
    /// `keyring` o `passphrase`: decide si la UI tiene que pedir contraseña.
    pub secrets_backend: secrets::Backend,
    pub unlocked: bool,
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> Result<AppInfo, AppError> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        secrets_backend: state.backend,
        unlocked: state.is_unlocked().await,
    })
}

/// Abre el almacén cifrado en las máquinas sin keyring.
#[tauri::command]
pub async fn unlock_secrets(
    state: State<'_, AppState>,
    passphrase: String,
) -> Result<(), AppError> {
    state.unlock(&passphrase).await?;
    Ok(())
}

/// Conecta una cuenta de Steam validando la clave contra la API antes de
/// guardarla: así un error de copiar y pegar se ve al momento y no como una
/// biblioteca vacía.
#[tauri::command]
pub async fn connect_steam(
    state: State<'_, AppState>,
    api_key: String,
    steam_id: String,
) -> Result<StoreAccountId, AppError> {
    let connector = state
        .connectors
        .get(&StoreId::Steam)
        .ok_or_else(|| AppError::Message("sin conector de Steam".to_owned()))?;

    let session = connector
        .authenticate(&AuthContext::ApiKey {
            key: api_key.trim().to_owned(),
            account_ref: steam_id.trim().to_owned(),
        })
        .await?;

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: session.account_ref.clone(),
        display_name: session.display_name.clone(),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(&state.db).upsert(&account).await?;

    // La credencial va al almacén de secretos. La base de datos solo sabe que
    // la cuenta existe.
    state
        .secrets()
        .await?
        .set(&credential_key(&account), &session.credential)?;

    Ok(id)
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<AccountView>, AppError> {
    Ok(StoreAccountRepository(&state.db)
        .active()
        .await?
        .into_iter()
        .map(|account| AccountView {
            store: account.store.as_str(),
            account_ref: account.account_ref,
            display_name: account.display_name,
            last_sync_at: account.last_sync_at.map(|t| t.unix_timestamp()),
        })
        .collect())
}

#[derive(Serialize)]
pub struct AccountView {
    pub store: &'static str,
    pub account_ref: String,
    pub display_name: Option<String>,
    pub last_sync_at: Option<i64>,
}

#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>) -> Result<SyncReport, AppError> {
    sync::sync_all(&state).await
}

#[derive(Serialize)]
pub struct LibrarySummary {
    pub owned: usize,
    pub wishlist: usize,
}

#[tauri::command]
pub async fn library_summary(state: State<'_, AppState>) -> Result<LibrarySummary, AppError> {
    let entries = StoreEntryRepository(&state.db);
    Ok(LibrarySummary {
        owned: entries.active(EntryKind::Owned).await?.len(),
        wishlist: entries.active(EntryKind::Wishlist).await?.len(),
    })
}
