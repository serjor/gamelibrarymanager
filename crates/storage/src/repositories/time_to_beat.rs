use std::collections::HashMap;

use domain::TimeToBeat;
use sqlx::Row;
use time::OffsetDateTime;

use crate::{Database, Result};

pub struct TimeToBeatRepository<'a>(pub &'a Database);

impl TimeToBeatRepository<'_> {
    /// The IGDB records of the library whose durations were never asked, or
    /// were asked before `stale_before`.
    ///
    /// The second group exists because the durations move: a new game has three
    /// answers in its first week and three hundred a year later.
    pub async fn due(&self, stale_before: OffsetDateTime) -> Result<Vec<i64>> {
        sqlx::query(
            "SELECT g.igdb_id FROM game g
               LEFT JOIN igdb_time_to_beat t ON t.igdb_id = g.igdb_id
              WHERE g.deleted_at IS NULL AND g.igdb_id IS NOT NULL
                AND (t.igdb_id IS NULL OR t.checked_at < ?)
              ORDER BY g.igdb_id",
        )
        .bind(stale_before.unix_timestamp())
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(|row| Ok(row.get("igdb_id")))
        .collect()
    }

    /// Writes the answer for each record that was asked, in one transaction.
    ///
    /// A record that was asked and is not in `found` gets a row with no
    /// duration: that is what IGDB said, and it is what stops the next pass from
    /// asking again at once. A row is replaced complete, never merged: a
    /// duration that IGDB no longer gives must not stay.
    pub async fn save(
        &self,
        asked: &[i64],
        found: &HashMap<i64, TimeToBeat>,
        now: OffsetDateTime,
    ) -> Result<()> {
        let mut tx = self.0.pool().begin().await?;
        for igdb_id in asked {
            let time = found.get(igdb_id);
            sqlx::query(
                "INSERT INTO igdb_time_to_beat
                     (igdb_id, hastily, normally, completely, submissions, checked_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT (igdb_id) DO UPDATE SET
                     hastily     = excluded.hastily,
                     normally    = excluded.normally,
                     completely  = excluded.completely,
                     submissions = excluded.submissions,
                     checked_at  = excluded.checked_at",
            )
            .bind(igdb_id)
            .bind(time.and_then(|t| t.hastily))
            .bind(time.and_then(|t| t.normally))
            .bind(time.and_then(|t| t.completely))
            .bind(time.map_or(0, |t| t.submissions))
            .bind(now.unix_timestamp())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
