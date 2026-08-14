use domain::{Game, GameId};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::{game_id_from_text, game_id_to_text};
use crate::{Database, Result};

pub struct GameRepository<'a>(pub &'a Database);

impl GameRepository<'_> {
    pub async fn upsert(&self, game: &Game) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query(
            "INSERT INTO game
                 (id, canonical_title, sort_title, igdb_id, cover_url, summary,
                  released_at, genres, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET
                 canonical_title = excluded.canonical_title,
                 sort_title      = excluded.sort_title,
                 igdb_id         = excluded.igdb_id,
                 cover_url       = excluded.cover_url,
                 summary         = excluded.summary,
                 released_at     = excluded.released_at,
                 genres          = excluded.genres,
                 updated_at      = excluded.updated_at,
                 deleted_at      = NULL",
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
        .execute(self.0.pool())
        .await?;
        Ok(())
    }

    pub async fn find(&self, id: GameId) -> Result<Option<Game>> {
        sqlx::query(
            "SELECT id, canonical_title, sort_title, igdb_id, cover_url, summary, released_at, genres
             FROM game WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(game_id_to_text(id))
        .fetch_optional(self.0.pool())
        .await?
        .as_ref()
        .map(hydrate)
        .transpose()
    }

    pub async fn find_by_igdb(&self, igdb_id: i64) -> Result<Option<Game>> {
        sqlx::query(
            "SELECT id, canonical_title, sort_title, igdb_id, cover_url, summary, released_at, genres
             FROM game WHERE igdb_id = ? AND deleted_at IS NULL",
        )
        .bind(igdb_id)
        .fetch_optional(self.0.pool())
        .await?
        .as_ref()
        .map(hydrate)
        .transpose()
    }

    pub async fn all(&self) -> Result<Vec<Game>> {
        sqlx::query(
            "SELECT id, canonical_title, sort_title, igdb_id, cover_url, summary, released_at, genres
             FROM game WHERE deleted_at IS NULL ORDER BY sort_title",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect()
    }
}

fn hydrate(row: &SqliteRow) -> Result<Game> {
    Ok(Game {
        id: game_id_from_text(&row.get::<String, _>("id"))?,
        canonical_title: row.get("canonical_title"),
        sort_title: row.get("sort_title"),
        igdb_id: row.get("igdb_id"),
        cover_url: row.get("cover_url"),
        summary: row.get("summary"),
        released_at: row.get("released_at"),
        genres: serde_json::from_str(&row.get::<String, _>("genres")).unwrap_or_default(),
    })
}
