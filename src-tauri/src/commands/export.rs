//! Export the library through the existing storage repositories.

use std::path::Path;

use domain::{ConnectorState, PlayStatus, StoreAccount};
use serde::{Deserialize, Serialize};
use storage::Database;
use storage::repositories::{
    ConnectorStateRepository, LibraryRepository, LibraryRow, StoreAccountRepository,
};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Csv,
}

#[derive(Serialize)]
struct JsonExport {
    library: Vec<LibraryRow>,
    accounts: Vec<StoreAccount>,
    connectors: Vec<ConnectorState>,
}

/// Writes the complete library to a path selected by the user.
#[tauri::command]
pub async fn export_library(
    state: State<'_, AppState>,
    path: String,
    format: ExportFormat,
) -> Result<(), AppError> {
    export_library_for(&state.db, Path::new(&path), format).await
}

/// The export use case without Tauri state. It keeps the file command easy to
/// test and makes the data sources explicit.
pub async fn export_library_for(
    db: &Database,
    path: &Path,
    format: ExportFormat,
) -> Result<(), AppError> {
    let rows = LibraryRepository(db).all().await?;
    let content = match format {
        ExportFormat::Json => {
            let export = JsonExport {
                library: rows,
                accounts: StoreAccountRepository(db).active().await?,
                connectors: ConnectorStateRepository(db).all().await?,
            };
            serde_json::to_string_pretty(&export)?
        }
        ExportFormat::Csv => csv(&rows),
    };

    std::fs::write(path, content)
        .map_err(|error| AppError::Message(format!("could not write the export: {error}")))?;
    Ok(())
}

fn csv(rows: &[LibraryRow]) -> String {
    let mut output = String::from("game_id,title,status,score,notes\n");
    for row in rows {
        let status = row.status.map(status_name).unwrap_or("");
        let score = row
            .rating
            .map(|rating| rating.to_string())
            .unwrap_or_default();
        output.push_str(&csv_field(&row.game_id.as_uuid().to_string()));
        output.push(',');
        output.push_str(&csv_field(&row.title));
        output.push(',');
        output.push_str(status);
        output.push(',');
        output.push_str(&score);
        output.push(',');
        output.push_str(&csv_field(row.notes.as_deref().unwrap_or("")));
        output.push('\n');
    }
    output
}

fn status_name(status: PlayStatus) -> &'static str {
    match status {
        PlayStatus::Backlog => "backlog",
        PlayStatus::Playing => "playing",
        PlayStatus::Finished => "finished",
        PlayStatus::Abandoned => "abandoned",
    }
}

fn csv_field(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}
