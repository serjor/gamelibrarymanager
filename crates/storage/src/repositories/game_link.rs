use domain::{GameId, GameLink, LinkMethod};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::{
    entry_id_from_text, entry_id_to_text, game_id_from_text, game_id_to_text, method_as_str,
    method_from_str,
};
use crate::{Database, Result};

pub struct GameLinkRepository<'a>(pub &'a Database);

impl GameLinkRepository<'_> {
    /// Makes the automatic matching again: it deletes the `auto` links and
    /// writes the new links in one transaction.
    ///
    /// The `manual` links are not touched. They are the word of the user and no
    /// algorithm examines them. `game` and `user_state` stay unchanged by
    /// design: this operation writes only in `game_link`.
    pub async fn rebuild_auto(&self, links: &[GameLink]) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let mut tx = self.0.pool().begin().await?;

        sqlx::query("DELETE FROM game_link WHERE method = 'auto'")
            .execute(&mut *tx)
            .await?;

        for link in links.iter().filter(|l| l.method == LinkMethod::Auto) {
            // If the entry already has a manual link, the unique index
            // protects it: the automatic proposal is ignored and does not
            // overwrite the manual link.
            sqlx::query(
                "INSERT OR IGNORE INTO game_link
                     (game_id, store_entry_id, confidence, method, updated_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(game_id_to_text(link.game_id))
            .bind(entry_id_to_text(link.store_entry_id))
            .bind(link.confidence)
            .bind(method_as_str(link.method))
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// A correction of the user: it replaces every link of that entry.
    pub async fn set_manual(&self, link: &GameLink) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let mut tx = self.0.pool().begin().await?;

        sqlx::query("DELETE FROM game_link WHERE store_entry_id = ?")
            .bind(entry_id_to_text(link.store_entry_id))
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "INSERT INTO game_link (game_id, store_entry_id, confidence, method, updated_at)
             VALUES (?, ?, ?, 'manual', ?)",
        )
        .bind(game_id_to_text(link.game_id))
        .bind(entry_id_to_text(link.store_entry_id))
        .bind(link.confidence)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn for_game(&self, game_id: GameId) -> Result<Vec<GameLink>> {
        sqlx::query(
            "SELECT game_id, store_entry_id, confidence, method
             FROM game_link WHERE game_id = ?",
        )
        .bind(game_id_to_text(game_id))
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect()
    }

    pub async fn all(&self) -> Result<Vec<GameLink>> {
        sqlx::query("SELECT game_id, store_entry_id, confidence, method FROM game_link")
            .fetch_all(self.0.pool())
            .await?
            .iter()
            .map(hydrate)
            .collect()
    }

    /// The store entries that found no record: the review queue.
    pub async fn unlinked_entry_count(&self) -> Result<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS n FROM store_entry e
             WHERE e.deleted_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM game_link l WHERE l.store_entry_id = e.id)",
        )
        .fetch_one(self.0.pool())
        .await?;
        Ok(row.get("n"))
    }
}

fn hydrate(row: &SqliteRow) -> Result<GameLink> {
    Ok(GameLink {
        game_id: game_id_from_text(&row.get::<String, _>("game_id"))?,
        store_entry_id: entry_id_from_text(&row.get::<String, _>("store_entry_id"))?,
        confidence: row.get("confidence"),
        method: method_from_str(&row.get::<String, _>("method"))?,
    })
}
