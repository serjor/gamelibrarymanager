//! Las entidades de la biblioteca, en el orden en que se apilan:
//! lo que dice la tienda, lo que deduce la app, la ficha y lo que escribe el
//! usuario. Cada una tiene un dueño distinto y ninguna pisa a la siguiente.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::model::{EntryKind, GameId, PlayStatus, StoreAccountId, StoreEntryId, StoreId};

/// Una cuenta conectada. Las credenciales no viven aquí: van al keyring, y esta
/// fila solo guarda a quién pertenecen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreAccount {
    pub id: StoreAccountId,
    pub store: StoreId,
    /// Identificador de la cuenta en la tienda (steamid, user id de GOG…).
    pub account_ref: String,
    pub display_name: Option<String>,
    pub connected_at: OffsetDateTime,
    pub last_sync_at: Option<OffsetDateTime>,
}

/// Lo que la tienda dice, tal cual. Nunca se edita a mano: la sincronización lo
/// da de alta o lo actualiza, y `raw` conserva la respuesta original para poder
/// re-emparejar en el futuro sin volver a preguntar a la tienda.
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
    pub raw: serde_json::Value,
}

/// La ficha unificada. Una por juego, aunque se posea en tres tiendas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub id: GameId,
    pub canonical_title: String,
    /// Título normalizado para ordenar y emparejar. Lo produce la fase 4.
    pub sort_title: String,
    pub igdb_id: Option<i64>,
    pub cover_url: Option<String>,
    pub summary: Option<String>,
    pub released_at: Option<OffsetDateTime>,
}

/// Cómo se decidió un enlace. `Manual` es la palabra del usuario y el
/// emparejamiento automático no puede sobrescribirla nunca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkMethod {
    Auto,
    Manual,
}

/// El resultado del emparejamiento, en su propia tabla para que rehacerlo no
/// toque ni el dato de la tienda ni el estado del usuario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameLink {
    pub game_id: GameId,
    pub store_entry_id: StoreEntryId,
    pub confidence: f64,
    pub method: LinkMethod,
}

/// Lo único que escribe el usuario. Ninguna sincronización lo toca.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserState {
    pub game_id: GameId,
    pub status: Option<PlayStatus>,
    pub rating: Option<u8>,
    pub notes: Option<String>,
    pub started_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
}
