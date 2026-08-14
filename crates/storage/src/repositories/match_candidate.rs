use domain::{ScoredCandidate, StoreEntryId};
use sqlx::Row;
use time::OffsetDateTime;

use crate::mapping::{entry_id_to_text, game_id_to_text};
use crate::{Database, Result};

/// Caché de la cola de revisión. Se puede vaciar entera sin perder nada: lo
/// único irrecuperable son los enlaces manuales, y esos viven en `game_link`.
pub struct MatchCandidateRepository<'a>(pub &'a Database);

impl MatchCandidateRepository<'_> {
    pub async fn replace(
        &self,
        entry_id: StoreEntryId,
        candidates: &[ScoredCandidate],
    ) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let mut tx = self.0.pool().begin().await?;

        sqlx::query("DELETE FROM match_candidate WHERE store_entry_id = ?")
            .bind(entry_id_to_text(entry_id))
            .execute(&mut *tx)
            .await?;

        for candidate in candidates {
            sqlx::query(
                "INSERT INTO match_candidate
                     (store_entry_id, igdb_id, name, score, release_year, cover_url, slug, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(entry_id_to_text(entry_id))
            .bind(candidate.igdb_id)
            .bind(&candidate.name)
            .bind(candidate.score)
            .bind(candidate.release_year)
            .bind(&candidate.cover_url)
            .bind(&candidate.slug)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn for_entry(&self, entry_id: StoreEntryId) -> Result<Vec<ScoredCandidate>> {
        Ok(sqlx::query(
            "SELECT igdb_id, name, score, release_year, cover_url, slug FROM match_candidate
             WHERE store_entry_id = ? ORDER BY score DESC, igdb_id",
        )
        .bind(entry_id_to_text(entry_id))
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(|row| ScoredCandidate {
            igdb_id: row.get("igdb_id"),
            name: row.get("name"),
            score: row.get("score"),
            release_year: row.get::<Option<i64>, _>("release_year").map(|y| y as i32),
            cover_url: row.get("cover_url"),
            slug: row.get("slug"),
        })
        .collect())
    }

    /// Al resolver una entrada sus candidatos sobran: sale de la cola.
    pub async fn clear(&self, entry_id: StoreEntryId) -> Result<()> {
        sqlx::query("DELETE FROM match_candidate WHERE store_entry_id = ?")
            .bind(entry_id_to_text(entry_id))
            .execute(self.0.pool())
            .await?;
        Ok(())
    }

    /// Fichas que ya no tiene ninguna entrada enlazada: quedan huérfanas cuando
    /// el usuario corrige un emparejamiento.
    pub async fn orphan_games(&self) -> Result<Vec<String>> {
        Ok(sqlx::query(
            "SELECT id FROM game g
             WHERE g.deleted_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM game_link l WHERE l.game_id = g.id)",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(|row| row.get::<String, _>("id"))
        .collect())
    }

    /// Baja lógica de una ficha huérfana. Nunca borra: si la ficha vuelve a
    /// hacer falta, el estado del usuario sigue colgando de ella.
    pub async fn soft_delete_game(&self, game_id: domain::GameId) -> Result<()> {
        sqlx::query("UPDATE game SET deleted_at = ?, updated_at = ? WHERE id = ?")
            .bind(OffsetDateTime::now_utc())
            .bind(OffsetDateTime::now_utc())
            .bind(game_id_to_text(game_id))
            .execute(self.0.pool())
            .await?;
        Ok(())
    }
}
