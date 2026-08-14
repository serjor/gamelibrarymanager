use domain::{EntryKind, StoreEntry, StoreEntryId};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::{
    account_id_from_text, account_id_to_text, entry_id_from_text, entry_id_to_text, kind_as_str,
    kind_from_str, store_from_str,
};
use crate::{Database, Result};

pub struct StoreEntryRepository<'a>(pub &'a Database);

impl StoreEntryRepository<'_> {
    /// Vuelca lo que ha dicho la tienda. Es idempotente a propósito: sincronizar
    /// dos veces actualiza, nunca duplica, y `first_seen_at` no se mueve.
    pub async fn upsert_many(&self, entries: &[StoreEntry]) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let mut tx = self.0.pool().begin().await?;

        for entry in entries {
            sqlx::query(
                "INSERT INTO store_entry
                     (id, account_id, store, store_app_id, kind, title, playtime_minutes,
                      acquired_at, raw, first_seen_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (account_id, store_app_id, kind) DO UPDATE SET
                     title            = excluded.title,
                     playtime_minutes = excluded.playtime_minutes,
                     acquired_at      = excluded.acquired_at,
                     raw              = excluded.raw,
                     updated_at       = excluded.updated_at,
                     deleted_at       = NULL",
            )
            .bind(entry_id_to_text(entry.id))
            .bind(account_id_to_text(entry.account_id))
            .bind(entry.store.as_str())
            .bind(&entry.store_app_id)
            .bind(kind_as_str(entry.kind))
            .bind(&entry.title)
            .bind(entry.playtime_minutes)
            .bind(entry.acquired_at)
            .bind(entry.raw.to_string())
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Baja lógica de lo que ya no aparece en la tienda. No se borra la fila:
    /// el juego puede volver, y su estado y su enlace siguen ahí cuando vuelva.
    pub async fn soft_delete_missing(
        &self,
        account_id: domain::StoreAccountId,
        kind: EntryKind,
        seen: &[String],
    ) -> Result<u64> {
        let placeholders = vec!["?"; seen.len()].join(",");
        let sql = format!(
            "UPDATE store_entry SET deleted_at = ?, updated_at = ?
             WHERE account_id = ? AND kind = ? AND deleted_at IS NULL
               AND store_app_id NOT IN ({placeholders})"
        );
        let now = OffsetDateTime::now_utc();
        let mut query = sqlx::query(&sql)
            .bind(now)
            .bind(now)
            .bind(account_id_to_text(account_id))
            .bind(kind_as_str(kind));
        for app_id in seen {
            query = query.bind(app_id);
        }
        Ok(query.execute(self.0.pool()).await?.rows_affected())
    }

    pub async fn active(&self, kind: EntryKind) -> Result<Vec<StoreEntry>> {
        sqlx::query(
            "SELECT id, account_id, store, store_app_id, kind, title, playtime_minutes,
                    acquired_at, raw
             FROM store_entry
             WHERE kind = ? AND deleted_at IS NULL
             ORDER BY title",
        )
        .bind(kind_as_str(kind))
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect()
    }

    /// Entradas activas que todavía no cuelgan de ninguna ficha. Es la cola de
    /// revisión y también lo que queda por emparejar.
    pub async fn unlinked(&self) -> Result<Vec<StoreEntry>> {
        sqlx::query(
            "SELECT e.id, e.account_id, e.store, e.store_app_id, e.kind, e.title,
                    e.playtime_minutes, e.acquired_at, e.raw
             FROM store_entry e
             WHERE e.deleted_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM game_link l WHERE l.store_entry_id = e.id)
             ORDER BY e.title",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect()
    }

    /// Entradas que cuelgan de una ficha creada sin metadatos.
    ///
    /// Son las que se emparejaron por título mientras IGDB no estaba
    /// configurado: siguen pendientes de identidad de verdad, pero ya se ven en
    /// la biblioteca, así que `unlinked` no las devuelve. Los enlaces manuales
    /// quedan fuera: la palabra del usuario no se revisa.
    pub async fn pending_metadata(&self) -> Result<Vec<StoreEntry>> {
        sqlx::query(
            "SELECT e.id, e.account_id, e.store, e.store_app_id, e.kind, e.title,
                    e.playtime_minutes, e.acquired_at, e.raw
             FROM store_entry e
             JOIN game_link l ON l.store_entry_id = e.id
             JOIN game g ON g.id = l.game_id
             WHERE e.deleted_at IS NULL
               AND g.deleted_at IS NULL
               AND g.igdb_id IS NULL
               AND l.method = 'auto'
             ORDER BY e.title",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect()
    }

    pub async fn find(&self, id: StoreEntryId) -> Result<Option<StoreEntry>> {
        sqlx::query(
            "SELECT id, account_id, store, store_app_id, kind, title, playtime_minutes,
                    acquired_at, raw
             FROM store_entry WHERE id = ?",
        )
        .bind(entry_id_to_text(id))
        .fetch_optional(self.0.pool())
        .await?
        .as_ref()
        .map(hydrate)
        .transpose()
    }
}

fn hydrate(row: &SqliteRow) -> Result<StoreEntry> {
    let raw: String = row.get("raw");
    Ok(StoreEntry {
        id: entry_id_from_text(&row.get::<String, _>("id"))?,
        account_id: account_id_from_text(&row.get::<String, _>("account_id"))?,
        store: store_from_str(&row.get::<String, _>("store"))?,
        store_app_id: row.get("store_app_id"),
        kind: kind_from_str(&row.get::<String, _>("kind"))?,
        title: row.get("title"),
        playtime_minutes: row.get("playtime_minutes"),
        acquired_at: row.get("acquired_at"),
        raw: serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null),
    })
}
