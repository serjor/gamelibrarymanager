use domain::{StoreAccount, StoreAccountId};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::{account_id_from_text, account_id_to_text, store_from_str};
use crate::{Database, Result};

pub struct StoreAccountRepository<'a>(pub &'a Database);

impl StoreAccountRepository<'_> {
    /// Alta o reconexión de una cuenta. Reconectar no crea una fila nueva: la
    /// biblioteca ya sincronizada sigue colgando de la misma cuenta.
    pub async fn upsert(&self, account: &StoreAccount) -> Result<StoreAccountId> {
        let now = OffsetDateTime::now_utc();
        let row = sqlx::query(
            "INSERT INTO store_account
                 (id, store, account_ref, display_name, connected_at, last_sync_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (store, account_ref) DO UPDATE SET
                 display_name = excluded.display_name,
                 updated_at   = excluded.updated_at,
                 deleted_at   = NULL
             RETURNING id",
        )
        .bind(account_id_to_text(account.id))
        .bind(account.store.as_str())
        .bind(&account.account_ref)
        .bind(&account.display_name)
        .bind(account.connected_at)
        .bind(account.last_sync_at)
        .bind(now)
        .fetch_one(self.0.pool())
        .await?;

        account_id_from_text(&row.get::<String, _>("id"))
    }

    pub async fn mark_synced(&self, id: StoreAccountId, at: OffsetDateTime) -> Result<()> {
        sqlx::query("UPDATE store_account SET last_sync_at = ?, updated_at = ? WHERE id = ?")
            .bind(at)
            .bind(at)
            .bind(account_id_to_text(id))
            .execute(self.0.pool())
            .await?;
        Ok(())
    }

    pub async fn active(&self) -> Result<Vec<StoreAccount>> {
        sqlx::query(
            "SELECT id, store, account_ref, display_name, connected_at, last_sync_at
             FROM store_account WHERE deleted_at IS NULL ORDER BY connected_at",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect()
    }
}

fn hydrate(row: &SqliteRow) -> Result<StoreAccount> {
    Ok(StoreAccount {
        id: account_id_from_text(&row.get::<String, _>("id"))?,
        store: store_from_str(&row.get::<String, _>("store"))?,
        account_ref: row.get("account_ref"),
        display_name: row.get("display_name"),
        connected_at: row.get("connected_at"),
        last_sync_at: row.get("last_sync_at"),
    })
}
