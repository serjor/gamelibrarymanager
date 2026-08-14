use domain::{GameId, PlayStatus};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::mapping::{game_id_from_text, status_from_str};
use crate::{Database, Result};

/// Una fila de la biblioteca: la ficha más todo lo que hay que enseñar de ella.
///
/// Se resuelve en una sola consulta a propósito. Pintar mil juegos haciendo una
/// consulta por juego para saber en qué tiendas está es la forma más fácil de
/// que la rejilla dé tirones.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LibraryRow {
    pub game_id: GameId,
    pub title: String,
    pub sort_title: String,
    pub cover_url: Option<String>,
    pub release_year: Option<i32>,
    pub genres: Vec<String>,
    pub owned_stores: Vec<String>,
    pub wishlist_stores: Vec<String>,
    pub playtime_minutes: i64,
    pub status: Option<PlayStatus>,
    pub rating: Option<u8>,
    pub notes: Option<String>,
}

pub struct LibraryRepository<'a>(pub &'a Database);

impl LibraryRepository<'_> {
    pub async fn all(&self) -> Result<Vec<LibraryRow>> {
        sqlx::query(
            "SELECT
                 g.id, g.canonical_title, g.sort_title, g.cover_url, g.released_at, g.genres,
                 us.status, us.rating, us.notes,
                 (SELECT GROUP_CONCAT(DISTINCT e.store) FROM game_link l
                    JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'owned' AND e.deleted_at IS NULL
                 ) AS owned_stores,
                 (SELECT GROUP_CONCAT(DISTINCT e.store) FROM game_link l
                    JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'wishlist' AND e.deleted_at IS NULL
                 ) AS wishlist_stores,
                 (SELECT COALESCE(SUM(e.playtime_minutes), 0) FROM game_link l
                    JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.deleted_at IS NULL
                 ) AS playtime_minutes
             FROM game g
             LEFT JOIN user_state us ON us.game_id = g.id
             WHERE g.deleted_at IS NULL
             ORDER BY g.sort_title",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect()
    }
}

fn hydrate(row: &SqliteRow) -> Result<LibraryRow> {
    let status: Option<String> = row.get("status");
    let released: Option<time::OffsetDateTime> = row.get("released_at");

    Ok(LibraryRow {
        game_id: game_id_from_text(&row.get::<String, _>("id"))?,
        title: row.get("canonical_title"),
        sort_title: row.get("sort_title"),
        cover_url: row.get("cover_url"),
        release_year: released.map(|date| date.year()),
        genres: serde_json::from_str(&row.get::<String, _>("genres")).unwrap_or_default(),
        owned_stores: split(row.get("owned_stores")),
        wishlist_stores: split(row.get("wishlist_stores")),
        playtime_minutes: row.get("playtime_minutes"),
        status: status.as_deref().map(status_from_str).transpose()?,
        rating: row.get::<Option<i64>, _>("rating").map(|r| r as u8),
        notes: row.get("notes"),
    })
}

/// `GROUP_CONCAT` devuelve NULL cuando no hay filas, no una cadena vacía, y no
/// garantiza el orden: sin ordenar aquí, las insignias de tienda de un juego
/// podrían cambiar de sitio entre dos aperturas de la aplicación.
fn split(value: Option<String>) -> Vec<String> {
    let mut items: Vec<String> = value
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    items.sort();
    items
}
