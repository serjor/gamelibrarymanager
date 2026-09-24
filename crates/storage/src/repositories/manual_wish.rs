use domain::{Game, GameId, ManualWish, ManualWishId, PlatformFamily};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::{
    family_from_str, game_id_from_text, game_id_to_text, wish_id_from_text, wish_id_to_text,
};
use crate::{Database, Result};

/// The wishes that the user writes. A connector never calls this repository.
pub struct ManualWishRepository<'a>(pub &'a Database);

impl ManualWishRepository<'_> {
    /// Creates a new record and its first wish together. A failed wish never
    /// leaves an empty record behind for the user to find later.
    pub async fn add_with_game(&self, game: &Game, wish: &ManualWish) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let mut tx = self.0.pool().begin().await?;
        sqlx::query(
            "INSERT INTO game
                 (id, canonical_title, sort_title, igdb_id, cover_url, summary,
                  released_at, genres, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(game_id_to_text(game.id))
        .bind(&game.canonical_title)
        .bind(&game.sort_title)
        .bind(game.igdb_id)
        .bind(&game.cover_url)
        .bind(&game.summary)
        .bind(game.released_at)
        .bind(serde_json::to_string(&game.genres).unwrap_or_else(|_| "[]".to_owned()))
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO manual_wish
                 (id, game_id, family, model, model_key, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(wish_id_to_text(wish.id))
        .bind(game_id_to_text(wish.game_id))
        .bind(wish.family.as_str())
        .bind(wish.model.trim())
        .bind(model_key(&wish.model))
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn add(&self, wish: &ManualWish) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query(
            "INSERT INTO manual_wish
                 (id, game_id, family, model, model_key, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(wish_id_to_text(wish.id))
        .bind(game_id_to_text(wish.game_id))
        .bind(wish.family.as_str())
        .bind(wish.model.trim())
        .bind(model_key(&wish.model))
        .bind(now)
        .bind(now)
        .execute(self.0.pool())
        .await?;
        Ok(())
    }

    pub async fn find(&self, id: ManualWishId) -> Result<Option<ManualWish>> {
        sqlx::query(
            "SELECT id, game_id, family, model FROM manual_wish
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(wish_id_to_text(id))
        .fetch_optional(self.0.pool())
        .await?
        .as_ref()
        .map(hydrate)
        .transpose()
    }

    pub async fn for_game(&self, game_id: GameId) -> Result<Vec<ManualWish>> {
        sqlx::query(
            "SELECT id, game_id, family, model FROM manual_wish
             WHERE game_id = ? AND deleted_at IS NULL ORDER BY family, model_key",
        )
        .bind(game_id_to_text(game_id))
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect()
    }

    pub async fn update_device(
        &self,
        id: ManualWishId,
        family: PlatformFamily,
        model: &str,
    ) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE manual_wish SET family = ?, model = ?, model_key = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(family.as_str())
        .bind(model.trim())
        .bind(model_key(model))
        .bind(OffsetDateTime::now_utc())
        .bind(wish_id_to_text(id))
        .execute(self.0.pool())
        .await?
        .rows_affected();
        Ok(changed > 0)
    }

    pub async fn remove(&self, id: ManualWishId) -> Result<bool> {
        let now = OffsetDateTime::now_utc();
        let changed = sqlx::query(
            "UPDATE manual_wish SET deleted_at = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(wish_id_to_text(id))
        .execute(self.0.pool())
        .await?
        .rows_affected();
        Ok(changed > 0)
    }
}

fn model_key(model: &str) -> String {
    model.trim().to_lowercase()
}

fn hydrate(row: &SqliteRow) -> Result<ManualWish> {
    Ok(ManualWish {
        id: wish_id_from_text(&row.get::<String, _>("id"))?,
        game_id: game_id_from_text(&row.get::<String, _>("game_id"))?,
        family: family_from_str(&row.get::<String, _>("family"))?,
        model: row.get("model"),
    })
}
