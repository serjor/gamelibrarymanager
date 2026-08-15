use domain::{ConnectorState, StoreId};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::store_from_str;
use crate::{Database, Result};

pub struct ConnectorStateRepository<'a>(pub &'a Database);

impl ConnectorStateRepository<'_> {
    /// State of every store that has one.
    ///
    /// A store with no row is on and with nothing wrong. Writing that default
    /// down would mean deciding on a first run which stores exist, and the
    /// answer changes with every phase of the plan.
    pub async fn all(&self) -> Result<Vec<ConnectorState>> {
        sqlx::query("SELECT store, enabled, last_error FROM connector_state ORDER BY store")
            .fetch_all(self.0.pool())
            .await?
            .iter()
            .map(hydrate)
            .collect()
    }

    /// Turns a connector on or off. The last error is left alone: it is what
    /// explains why the switch was touched.
    pub async fn set_enabled(&self, store: StoreId, enabled: bool) -> Result<()> {
        sqlx::query(
            "INSERT INTO connector_state (store, enabled, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT (store) DO UPDATE SET
                 enabled    = excluded.enabled,
                 updated_at = excluded.updated_at",
        )
        .bind(store.as_str())
        .bind(enabled)
        .bind(OffsetDateTime::now_utc())
        .execute(self.0.pool())
        .await?;
        Ok(())
    }

    /// Writes down what went wrong, or clears it when the run went well.
    ///
    /// It never touches `enabled`: recovering from an error does not turn a
    /// connector the user switched off back on.
    ///
    /// Clearing is an update and never an insert. A store that works and has
    /// never failed keeps no row, which is what makes the absence of a row mean
    /// something instead of being one more state to read.
    pub async fn record_error(&self, store: StoreId, error: Option<&str>) -> Result<()> {
        let query = match error {
            Some(reason) => sqlx::query(
                "INSERT INTO connector_state (store, enabled, last_error, updated_at)
                 VALUES (?, 1, ?, ?)
                 ON CONFLICT (store) DO UPDATE SET
                     last_error = excluded.last_error,
                     updated_at = excluded.updated_at",
            )
            .bind(store.as_str())
            .bind(reason)
            .bind(OffsetDateTime::now_utc()),

            None => sqlx::query(
                "UPDATE connector_state SET last_error = NULL, updated_at = ? WHERE store = ?",
            )
            .bind(OffsetDateTime::now_utc())
            .bind(store.as_str()),
        };

        query.execute(self.0.pool()).await?;
        Ok(())
    }
}

fn hydrate(row: &SqliteRow) -> Result<ConnectorState> {
    Ok(ConnectorState {
        store: store_from_str(&row.get::<String, _>("store"))?,
        enabled: row.get("enabled"),
        last_error: row.get("last_error"),
    })
}
