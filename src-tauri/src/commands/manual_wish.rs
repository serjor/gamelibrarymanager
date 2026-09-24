//! User-owned wishes. A store sync never writes these rows.

use domain::{Game, GameId, ManualWish, ManualWishId, PlatformFamily, matching};
use metadata::igdb::GameMetadata;
use serde::Deserialize;
use storage::Database;
use storage::repositories::{GameRepository, LibraryRepository, LibraryRow, ManualWishRepository};
use tauri::State;
use uuid::Uuid;

use super::igdb_session;
use crate::error::AppError;
use crate::state::AppState;

pub enum WishTarget {
    Existing(GameId),
    New(Game),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WishTargetInput {
    Existing { game_id: String },
    Igdb { igdb_id: i64 },
    Title { title: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddWishInput {
    target: WishTargetInput,
    family: PlatformFamily,
    model: String,
}

#[tauri::command]
pub async fn search_manual_wish_games(
    state: State<'_, AppState>,
    title: String,
) -> Result<Vec<domain::Candidate>, AppError> {
    let title = valid_title(&title)?;
    let (credentials, token) = igdb_session(&state).await?;
    Ok(state.igdb.search(&credentials, &token, &title).await?)
}

#[tauri::command]
pub async fn add_manual_wish(
    state: State<'_, AppState>,
    input: AddWishInput,
) -> Result<LibraryRow, AppError> {
    let target = match input.target {
        WishTargetInput::Existing { game_id } => WishTarget::Existing(parse_game_id(&game_id)?),
        WishTargetInput::Title { title } => WishTarget::New(local_game(&valid_title(&title)?)),
        WishTargetInput::Igdb { igdb_id } => {
            if igdb_id <= 0 {
                return Err(AppError::Message("invalid IGDB identifier".to_owned()));
            }
            if let Some(existing) = GameRepository(&state.db).find_by_igdb(igdb_id).await? {
                WishTarget::Existing(existing.id)
            } else {
                let (credentials, token) = igdb_session(&state).await?;
                let metadata = state
                    .igdb
                    .game(&credentials, &token, igdb_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Message("that IGDB game is no longer available".to_owned())
                    })?;
                WishTarget::New(game_from_metadata(metadata))
            }
        }
    };
    add_manual_wish_for(&state.db, target, input.family, &input.model).await
}

#[tauri::command]
pub async fn update_manual_wish(
    state: State<'_, AppState>,
    wish_id: String,
    family: PlatformFamily,
    model: String,
) -> Result<LibraryRow, AppError> {
    update_manual_wish_for(&state.db, parse_wish_id(&wish_id)?, family, &model).await
}

#[tauri::command]
pub async fn remove_manual_wish(
    state: State<'_, AppState>,
    wish_id: String,
) -> Result<LibraryRow, AppError> {
    remove_manual_wish_for(&state.db, parse_wish_id(&wish_id)?).await
}

pub async fn add_manual_wish_for(
    db: &Database,
    target: WishTarget,
    family: PlatformFamily,
    model: &str,
) -> Result<LibraryRow, AppError> {
    let model = valid_model(family, model)?;
    let repo = ManualWishRepository(db);
    let game_id = match target {
        WishTarget::Existing(id) => {
            if GameRepository(db).find(id).await?.is_none() {
                return Err(AppError::Message(
                    "that game is no longer in the library".to_owned(),
                ));
            }
            let wish = ManualWish {
                id: ManualWishId::new(),
                game_id: id,
                family,
                model,
            };
            repo.add(&wish).await.map_err(friendly_storage_error)?;
            id
        }
        WishTarget::New(game) => {
            let wish = ManualWish {
                id: ManualWishId::new(),
                game_id: game.id,
                family,
                model,
            };
            repo.add_with_game(&game, &wish)
                .await
                .map_err(friendly_storage_error)?;
            game.id
        }
    };
    row(db, game_id).await
}

pub async fn update_manual_wish_for(
    db: &Database,
    id: ManualWishId,
    family: PlatformFamily,
    model: &str,
) -> Result<LibraryRow, AppError> {
    let model = valid_model(family, model)?;
    let repo = ManualWishRepository(db);
    let wish = repo.find(id).await?.ok_or_else(missing_wish)?;
    if !repo
        .update_device(id, family, &model)
        .await
        .map_err(friendly_storage_error)?
    {
        return Err(missing_wish());
    }
    row(db, wish.game_id).await
}

pub async fn remove_manual_wish_for(
    db: &Database,
    id: ManualWishId,
) -> Result<LibraryRow, AppError> {
    let repo = ManualWishRepository(db);
    let wish = repo.find(id).await?.ok_or_else(missing_wish)?;
    if !repo.remove(id).await? {
        return Err(missing_wish());
    }
    row(db, wish.game_id).await
}

async fn row(db: &Database, id: GameId) -> Result<LibraryRow, AppError> {
    LibraryRepository(db)
        .one(id)
        .await?
        .ok_or_else(|| AppError::Message("that game is no longer in the library".to_owned()))
}

fn local_game(title: &str) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: matching::normalize(title),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    }
}

fn game_from_metadata(meta: GameMetadata) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: meta.name.clone(),
        sort_title: matching::normalize(&meta.name),
        igdb_id: Some(meta.igdb_id),
        cover_url: meta.cover_url,
        summary: meta.summary,
        released_at: meta.released_at,
        genres: meta.genres,
    }
}

fn valid_title(raw: &str) -> Result<String, AppError> {
    let title = raw.trim();
    if title.is_empty() || title.chars().count() > 200 || matching::normalize(title).is_empty() {
        return Err(AppError::Message(
            "enter a game title of at most 200 characters".to_owned(),
        ));
    }
    Ok(title.to_owned())
}

fn valid_model(family: PlatformFamily, raw: &str) -> Result<String, AppError> {
    let model = raw.trim();
    if model.chars().count() > 80 || (family == PlatformFamily::Other && model.is_empty()) {
        return Err(AppError::Message(
            "enter a device model of at most 80 characters".to_owned(),
        ));
    }
    Ok(model.to_owned())
}

fn parse_game_id(raw: &str) -> Result<GameId, AppError> {
    Uuid::parse_str(raw)
        .map(GameId::from_uuid)
        .map_err(|_| AppError::Message("invalid game identifier".to_owned()))
}

fn parse_wish_id(raw: &str) -> Result<ManualWishId, AppError> {
    Uuid::parse_str(raw)
        .map(ManualWishId::from_uuid)
        .map_err(|_| AppError::Message("invalid wish identifier".to_owned()))
}

fn missing_wish() -> AppError {
    AppError::Message("that wish is no longer in the library".to_owned())
}

fn friendly_storage_error(error: storage::StorageError) -> AppError {
    if let storage::StorageError::Database(ref sql_error) = error
        && sql_error
            .as_database_error()
            .is_some_and(|database_error| database_error.is_unique_violation())
    {
        return AppError::Message("that device is already in the wishlist".to_owned());
    }
    AppError::Storage(error)
}
