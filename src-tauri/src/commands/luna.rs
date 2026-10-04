//! The commands of Luna. They control the use case in `crate::luna`.

use metadata::luna::LunaRegion;
use serde::Serialize;
use storage::repositories::{LunaRepository, LunaSettings};
use tauri::State;

use crate::error::AppError;
use crate::luna::{self, LunaReport};
use crate::state::AppState;

/// What the card of Luna in Utilities shows.
#[derive(Serialize)]
pub struct LunaView {
    /// `None` when Luna is switched off.
    pub settings: Option<LunaSettings>,
    /// The countries that the user can select.
    pub countries: Vec<&'static str>,
}

#[tauri::command]
pub async fn luna_settings(state: State<'_, AppState>) -> Result<LunaView, AppError> {
    Ok(LunaView {
        settings: LunaRepository(&state.db).settings().await?,
        countries: LunaRegion::supported()
            .iter()
            .map(|region| region.country)
            .collect(),
    })
}

/// Switches Luna on, or changes the country, and asks for the catalogue.
#[tauri::command]
pub async fn set_luna_country(
    state: State<'_, AppState>,
    country: String,
) -> Result<LunaReport, AppError> {
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    luna::enable(&state.db, &state.luna, &country).await
}

#[tauri::command]
pub async fn disable_luna(state: State<'_, AppState>) -> Result<(), AppError> {
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    luna::disable(&state.db).await
}

/// Asks for the catalogue again.
///
/// It has a button of its own and it is not a step of the synchronisation. It
/// takes the guard of the long operations, as the prices do: a refresh while a
/// synchronisation writes would be a second writer for nothing.
#[tauri::command]
pub async fn refresh_luna(state: State<'_, AppState>) -> Result<LunaReport, AppError> {
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    luna::refresh(&state.db, &state.luna).await
}
