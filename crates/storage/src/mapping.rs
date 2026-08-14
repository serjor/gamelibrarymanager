//! Traducción entre el dominio y la representación en SQLite. La ortografía que
//! espera la base de datos vive aquí y no en el dominio.

use domain::{EntryKind, GameId, LinkMethod, PlayStatus, StoreAccountId, StoreEntryId, StoreId};
use uuid::Uuid;

use crate::StorageError;

pub(crate) fn store_from_str(value: &str) -> Result<StoreId, StorageError> {
    match value {
        "steam" => Ok(StoreId::Steam),
        "gog" => Ok(StoreId::Gog),
        "epic" => Ok(StoreId::Epic),
        other => Err(corrupt("store", other)),
    }
}

pub(crate) fn kind_as_str(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Owned => "owned",
        EntryKind::Wishlist => "wishlist",
    }
}

pub(crate) fn kind_from_str(value: &str) -> Result<EntryKind, StorageError> {
    match value {
        "owned" => Ok(EntryKind::Owned),
        "wishlist" => Ok(EntryKind::Wishlist),
        other => Err(corrupt("kind", other)),
    }
}

pub(crate) fn method_as_str(method: LinkMethod) -> &'static str {
    match method {
        LinkMethod::Auto => "auto",
        LinkMethod::Manual => "manual",
    }
}

pub(crate) fn method_from_str(value: &str) -> Result<LinkMethod, StorageError> {
    match value {
        "auto" => Ok(LinkMethod::Auto),
        "manual" => Ok(LinkMethod::Manual),
        other => Err(corrupt("method", other)),
    }
}

pub(crate) fn status_as_str(status: PlayStatus) -> &'static str {
    match status {
        PlayStatus::Backlog => "backlog",
        PlayStatus::Playing => "playing",
        PlayStatus::Finished => "finished",
        PlayStatus::Abandoned => "abandoned",
    }
}

pub(crate) fn status_from_str(value: &str) -> Result<PlayStatus, StorageError> {
    match value {
        "backlog" => Ok(PlayStatus::Backlog),
        "playing" => Ok(PlayStatus::Playing),
        "finished" => Ok(PlayStatus::Finished),
        "abandoned" => Ok(PlayStatus::Abandoned),
        other => Err(corrupt("status", other)),
    }
}

macro_rules! id_mapping {
    ($to:ident, $from:ident, $ty:ty, $column:literal) => {
        pub(crate) fn $to(id: $ty) -> String {
            id.as_uuid().to_string()
        }

        pub(crate) fn $from(value: &str) -> Result<$ty, StorageError> {
            Uuid::parse_str(value)
                .map(<$ty>::from_uuid)
                .map_err(|_| corrupt($column, value))
        }
    };
}

id_mapping!(game_id_to_text, game_id_from_text, GameId, "game_id");
id_mapping!(
    entry_id_to_text,
    entry_id_from_text,
    StoreEntryId,
    "store_entry_id"
);
id_mapping!(
    account_id_to_text,
    account_id_from_text,
    StoreAccountId,
    "account_id"
);

fn corrupt(column: &'static str, value: &str) -> StorageError {
    StorageError::Corrupt {
        column,
        value: value.to_owned(),
    }
}
