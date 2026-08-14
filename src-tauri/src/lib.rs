mod commands;
mod error;
mod identity;
mod state;
mod sync;

/// Superficie que consumen los tests de integración. No la usa la aplicación:
/// existe para poder probar el caso de uso completo sin arrancar Tauri.
pub mod testing {
    pub use crate::identity::{IdentityReport, resolve};
    pub use crate::state::credential_key;
    pub use crate::sync::{SyncReport, sync_account};
}

use state::AppState;
use storage::Database;
use tauri::Manager;

/// Arranca la aplicación. Los comandos solo orquestan casos de uso: la lógica
/// vive en los crates de dominio y adaptadores, nunca aquí.
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
            commands::list_accounts,
            commands::sync_now,
            commands::library_summary,
            commands::set_igdb_credentials,
            commands::has_igdb_credentials,
            commands::resolve_identities,
            commands::review_queue,
            commands::review_confirm,
            commands::review_without_metadata,
        ])
        .run(tauri::generate_context!())
        .expect("error al arrancar la aplicación");
}
