//! The skins that other persons write, read from a folder.
//!
//! This command only reads files and gives their text to the interface. It does
//! not parse the JSON: `validateSkin` in `src/features/shell/skin.ts` examines
//! each skin, with the same rules as the included skins, and the list of tokens
//! lives next to the stylesheet. What Rust decides is what the interface never
//! sees: only the regular `.json` files of the first level, a limit of files and
//! a limit of size, and text in UTF-8. See the plan
//! `.agents/plans/0009-skins/plan.html`, phase 3.

use std::fs;
use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::AppError;

/// The folder, inside the data directory: the one that holds `library.db`.
pub const SKINS_DIR: &str = "skins";
/// More files than this is not a collection of skins.
pub const MAX_SKIN_FILES: usize = 50;
/// A skin with every token is about 4 KB. 64 KB leaves space for comments of
/// the author and stops a file that is not a skin.
pub const MAX_SKIN_BYTES: u64 = 64 * 1024;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct UserSkins {
    /// The folder, to show it and to open it.
    pub dir: String,
    pub files: Vec<UserSkinFile>,
}

/// One file of the folder: its text, or why it was not read.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct UserSkinFile {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl UserSkinFile {
    fn failed(file: String, error: impl Into<String>) -> Self {
        Self {
            file,
            text: None,
            error: Some(error.into()),
        }
    }
}

/// Reads the skin files of a folder, and makes the folder if it does not exist.
///
/// A file that does not obey the limits comes back with its error and does not
/// stop the others: the author sees which file and why.
pub fn read_skins_dir(dir: &Path) -> Result<UserSkins, AppError> {
    fs::create_dir_all(dir)
        .map_err(|e| AppError::Message(format!("cannot make the skins folder: {e}")))?;

    let mut names = Vec::new();
    for entry in fs::read_dir(dir)
        .map_err(|e| AppError::Message(format!("cannot read the skins folder: {e}")))?
    {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_ascii_lowercase().ends_with(".json") {
            names.push(name);
        }
    }
    names.sort();

    let files = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            if index >= MAX_SKIN_FILES {
                return UserSkinFile::failed(
                    name,
                    format!(
                        "the folder holds more than {MAX_SKIN_FILES} files: this one is not read"
                    ),
                );
            }
            read_one(&dir.join(&name), name)
        })
        .collect();

    Ok(UserSkins {
        dir: dir.to_string_lossy().into_owned(),
        files,
    })
}

fn read_one(path: &Path, name: String) -> UserSkinFile {
    // `symlink_metadata` does not follow a link: a link can point to any file of
    // the machine, and the folder is for skins only.
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) => return UserSkinFile::failed(name, format!("cannot read it: {e}")),
    };
    if metadata.file_type().is_symlink() {
        return UserSkinFile::failed(name, "it is a link: put the file itself in the folder");
    }
    if !metadata.is_file() {
        return UserSkinFile::failed(name, "it is not a file");
    }
    if metadata.len() > MAX_SKIN_BYTES {
        return UserSkinFile::failed(
            name,
            format!("it has more than {} KB", MAX_SKIN_BYTES / 1024),
        );
    }
    match fs::read(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => UserSkinFile {
                file: name,
                text: Some(text),
                error: None,
            },
            Err(_) => UserSkinFile::failed(name, "it is not UTF-8 text"),
        },
        Err(e) => UserSkinFile::failed(name, format!("cannot read it: {e}")),
    }
}

#[tauri::command]
pub fn list_user_skins(app: AppHandle) -> Result<UserSkins, AppError> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(format!("there is no data directory: {e}")))?;
    read_skins_dir(&data.join(SKINS_DIR))
}
