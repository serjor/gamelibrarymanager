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
    /// Writes what the store said. It is idempotent, and that is deliberate: a
    /// second synchronisation updates, it never duplicates, and `first_seen_at`
    /// does not change.
    pub async fn upsert_many(&self, entries: &[StoreEntry]) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let mut tx = self.0.pool().begin().await?;

        for entry in entries {
            sqlx::query(
                "INSERT INTO store_entry
                     (id, account_id, store, store_app_id, kind, title, playtime_minutes,
                      acquired_at, cover_url, store_url, raw, first_seen_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (account_id, store_app_id, kind) DO UPDATE SET
                     title            = excluded.title,
                     playtime_minutes = excluded.playtime_minutes,
                     acquired_at      = excluded.acquired_at,
                     cover_url        = excluded.cover_url,
                     store_url        = excluded.store_url,
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
            .bind(&entry.cover_url)
            .bind(&entry.store_url)
            .bind(entry.raw.to_string())
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// How many identifiers go in each `INSERT` into the temporary table.
    ///
    /// SQLite accepts a limited number of parameters in one statement, and a
    /// library of five thousand copies passes it. This number divides the
    /// **writing** of the list, which is safe. What is never divided is the
    /// comparison: see `soft_delete_missing`.
    const SEEN_BATCH: usize = 500;

    /// A logical delete of the entries that the store no longer shows. The row
    /// is not deleted: the game can come back, and its status and its link are
    /// still there when it comes back.
    ///
    /// The identifiers that the store gave go into a temporary table, and the
    /// comparison is made against that table. It is not a list of placeholders:
    /// with thousands of copies that statement passes the limit of parameters of
    /// SQLite, and to divide the `NOT IN` into batches is worse than a failure —
    /// each batch would delete everything that is in the other batches.
    ///
    /// All of it is one transaction, thus a failure in the middle leaves the
    /// library exactly as it was.
    pub async fn soft_delete_missing(
        &self,
        account_id: domain::StoreAccountId,
        kind: EntryKind,
        seen: &[String],
    ) -> Result<u64> {
        let now = OffsetDateTime::now_utc();
        let mut tx = self.0.pool().begin().await?;

        // The pool gives one connection and it is used again, thus a table that
        // a failure before left behind would still be here with its rows.
        sqlx::query("DROP TABLE IF EXISTS temp.seen")
            .execute(&mut *tx)
            .await?;
        sqlx::query("CREATE TEMP TABLE seen (app_id TEXT PRIMARY KEY)")
            .execute(&mut *tx)
            .await?;

        for batch in seen.chunks(Self::SEEN_BATCH) {
            let values = vec!["(?)"; batch.len()].join(",");
            // `OR IGNORE` because a store can name the same copy two times, and
            // that is not a reason to stop a synchronisation.
            let sql = format!("INSERT OR IGNORE INTO seen (app_id) VALUES {values}");
            let mut insert = sqlx::query(&sql);
            for app_id in batch {
                insert = insert.bind(app_id);
            }
            insert.execute(&mut *tx).await?;
        }

        let deleted = sqlx::query(
            "UPDATE store_entry SET deleted_at = ?, updated_at = ?
             WHERE account_id = ? AND kind = ? AND deleted_at IS NULL
               AND store_app_id NOT IN (SELECT app_id FROM seen)",
        )
        .bind(now)
        .bind(now)
        .bind(account_id_to_text(account_id))
        .bind(kind_as_str(kind))
        .execute(&mut *tx)
        .await?
        .rows_affected();

        sqlx::query("DROP TABLE seen").execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(deleted)
    }

    /// The copies of one kind that the stores still show.
    ///
    /// A count and not a list: the summary showed four numbers, and to build
    /// them it read every row and parsed every `raw` JSON to throw all of it
    /// away.
    pub async fn count_active(&self, kind: EntryKind) -> Result<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS n FROM store_entry WHERE kind = ? AND deleted_at IS NULL",
        )
        .bind(kind_as_str(kind))
        .fetch_one(self.0.pool())
        .await?;
        Ok(row.get("n"))
    }

    pub async fn active(&self, kind: EntryKind) -> Result<Vec<StoreEntry>> {
        sqlx::query(
            "SELECT id, account_id, store, store_app_id, kind, title, playtime_minutes,
                    acquired_at, cover_url, store_url, raw
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

    /// The active entries that are not yet attached to a record. This is the
    /// review queue and also the work that the matching has left.
    pub async fn unlinked(&self) -> Result<Vec<StoreEntry>> {
        sqlx::query(
            "SELECT e.id, e.account_id, e.store, e.store_app_id, e.kind, e.title,
                    e.playtime_minutes, e.acquired_at, e.cover_url, e.store_url, e.raw
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

    /// The entries that are attached to a record made with no metadata.
    ///
    /// These are the entries matched by title while IGDB was not configured:
    /// they are still without a true identity, but you can already see them in
    /// the library, thus `unlinked` does not give them back. The manual links
    /// stay out: the word of the user is not examined again.
    pub async fn pending_metadata(&self) -> Result<Vec<StoreEntry>> {
        sqlx::query(
            "SELECT e.id, e.account_id, e.store, e.store_app_id, e.kind, e.title,
                    e.playtime_minutes, e.acquired_at, e.cover_url, e.store_url, e.raw
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
                    acquired_at, cover_url, store_url, raw
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
        cover_url: row.get("cover_url"),
        store_url: row.get("store_url"),
        raw: serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null),
    })
}
