mod commands;
mod error;
mod identity;
mod prices;
mod state;
mod sync;

/// The surface that the integration tests use. The application does not use it:
/// it exists so that you can test the complete use case without you start
/// Tauri.
pub mod testing {
    pub use crate::commands::{LibrarySummary, summary};
    pub use crate::error::AppError;
    pub use crate::identity::{IdentityReport, resolve, resolve_local};
    pub use crate::prices::{PriceReport, refresh as refresh_prices};
    pub use crate::state::{
        AppState, OperationGuard, credential_key, http_client, http_client_with,
    };
    pub use crate::sync::{Silent, SyncReport, sync_account, sync_all, sync_stores};
}

use state::AppState;
use storage::Database;
use tauri::Manager;

/// Starts the application. The commands only control the use cases: the logic
/// lives in the domain and adapter crates, never here.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;

            let db = tauri::async_runtime::block_on(Database::open(&dir.join("library.db")))?;
            app.manage(AppState::new(db, dir.join("secrets.bin")));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::unlock_secrets,
            commands::connect_steam,
            commands::gog::connect_gog,
            commands::epic::connect_epic,
            commands::list_accounts,
            commands::connector_states,
            commands::set_connector_enabled,
            commands::sync_now,
            commands::library_summary,
            commands::set_igdb_credentials,
            commands::has_igdb_credentials,
            commands::set_itad_credentials,
            commands::has_itad_credentials,
            commands::refresh_prices,
            commands::prices,
            commands::resolve_identities,
            commands::review_queue,
            commands::review_confirm,
            commands::review_confirm_many,
            commands::review_without_metadata,
            commands::cancel_operation,
            commands::library,
            commands::set_user_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while the application started");
}
