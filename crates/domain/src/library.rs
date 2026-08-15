//! The library entities, in the order in which they stack up: what the store
//! says, what the application deduces, the metadata record and what the user
//! writes. Each one has a different owner and none overwrites the next.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::model::{EntryKind, GameId, PlayStatus, StoreAccountId, StoreEntryId, StoreId};

/// A connected account. The credentials do not live here: they go to the
/// keyring, and this row only keeps a record of who owns them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreAccount {
    pub id: StoreAccountId,
    pub store: StoreId,
    /// The identifier of the account in the store (steamid, GOG user id, and so
    /// on).
    pub account_ref: String,
    pub display_name: Option<String>,
    pub connected_at: OffsetDateTime,
    pub last_sync_at: Option<OffsetDateTime>,
}

/// What the store says, unchanged. It is never edited by hand: the
/// synchronisation adds it or updates it, and `raw` keeps the initial answer so
/// that you can match again in the future without a new request to the store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreEntry {
    pub id: StoreEntryId,
    pub account_id: StoreAccountId,
    pub store: StoreId,
    pub store_app_id: String,
    pub kind: EntryKind,
    pub title: String,
    pub playtime_minutes: Option<i64>,
    pub acquired_at: Option<OffsetDateTime>,
    /// The cover and the page of the copy **in its own store**. The matching
    /// does not use them: they exist so that the user can compare what the
    /// store says with what IGDB proposes before they accept an unsure link.
    pub cover_url: Option<String>,
    pub store_url: Option<String>,
    pub raw: serde_json::Value,
}

/// The unified metadata record. One for each game, even if you own it in three
/// stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub id: GameId,
    pub canonical_title: String,
    /// The title, normalised for sorting and matching. Phase 4 produces it.
    pub sort_title: String,
    pub igdb_id: Option<i64>,
    pub cover_url: Option<String>,
    pub summary: Option<String>,
    pub released_at: Option<OffsetDateTime>,
    pub genres: Vec<String>,
}

/// How a link was decided. `Manual` is the word of the user, and the automatic
/// matching can never overwrite it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkMethod {
    Auto,
    Manual,
}

/// The result of the matching, in a table of its own so that a new match does
/// not touch the store data or the user status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameLink {
    pub game_id: GameId,
    pub store_entry_id: StoreEntryId,
    pub confidence: f64,
    pub method: LinkMethod,
}

/// The only data that the user writes. No synchronisation touches it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserState {
    pub game_id: GameId,
    pub status: Option<PlayStatus>,
    pub rating: Option<u8>,
    pub notes: Option<String>,
    pub started_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
}
