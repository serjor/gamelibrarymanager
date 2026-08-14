use domain::{GameId, UserState};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::{game_id_from_text, game_id_to_text, status_as_str, status_from_str};
use crate::{Database, Result};

/// Lo único que escribe el usuario. Ninguna sincronización ni emparejamiento
/// pasa por aquí: por eso vive en su propia tabla y en su propio repositorio.
pub struct UserStateRepository<'a>(pub &'a Database);

impl UserStateRepository<'_> {
    pub async fn save(&self, state: &UserState) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query(
            "INSERT INTO user_state
                 (game_id, status, rating, notes, started_at, finished_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (game_id) DO UPDATE SET
                 status      = excluded.status,
                 rating      = excluded.rating,
                 notes       = excluded.notes,
                 started_at  = excluded.started_at,
                 finished_at = excluded.finished_at,
                 updated_at  = excluded.updated_at",
        )
        .bind(game_id_to_text(state.game_id))
        .bind(state.status.map(status_as_str))
        .bind(state.rating)
        .bind(&state.notes)
        .bind(state.started_at)
        .bind(state.finished_at)
        .bind(now)
        .execute(self.0.pool())
        .await?;
        Ok(())
    }

    pub async fn find(&self, game_id: GameId) -> Result<Option<UserState>> {
        sqlx::query(
            "SELECT game_id, status, rating, notes, started_at, finished_at
             FROM user_state WHERE game_id = ?",
        )
        .bind(game_id_to_text(game_id))
        .fetch_optional(self.0.pool())
        .await?
        .as_ref()
        .map(hydrate)
        .transpose()
    }
}

fn hydrate(row: &SqliteRow) -> Result<UserState> {
    let status: Option<String> = row.get("status");
    Ok(UserState {
        game_id: game_id_from_text(&row.get::<String, _>("game_id"))?,
        status: status.as_deref().map(status_from_str).transpose()?,
        rating: row.get::<Option<i64>, _>("rating").map(|r| r as u8),
        notes: row.get("notes"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    })
}
