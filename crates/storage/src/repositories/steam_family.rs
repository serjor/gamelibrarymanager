use std::collections::HashMap;

use domain::{FamilyApp, FamilyGroup, FamilyOwner};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::{Database, Result, StorageError};

/// What Utilities shows about the family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SteamFamilySettings {
    /// `None` before the first stage that ended, and when Steam says that the
    /// user is in no family.
    pub family_groupid: Option<String>,
    /// Seconds from the epoch. `None` until the first stage ends.
    pub refreshed_at: Option<i64>,
    /// Why the last stage did not end. It is cleared when a stage ends.
    pub last_error: Option<String>,
    /// How many games the family can lend now.
    pub games: i64,
}

pub struct SteamFamilyRepository<'a>(pub &'a Database);

impl SteamFamilyRepository<'_> {
    /// The settings, or `None` if the family is switched off.
    pub async fn settings(&self) -> Result<Option<SteamFamilySettings>> {
        Ok(sqlx::query(
            "SELECT family_groupid, refreshed_at, last_error,
                    (SELECT COUNT(*) FROM steam_family_app) AS games
               FROM steam_family WHERE id = 1",
        )
        .fetch_optional(self.0.pool())
        .await?
        .map(|row| SteamFamilySettings {
            family_groupid: row.get("family_groupid"),
            refreshed_at: row.get("refreshed_at"),
            last_error: row.get("last_error"),
            games: row.get("games"),
        }))
    }

    /// Switches the family on. A family that is already on keeps its list.
    pub async fn enable(&self) -> Result<()> {
        sqlx::query("INSERT INTO steam_family (id) VALUES (1) ON CONFLICT (id) DO NOTHING")
            .execute(self.0.pool())
            .await?;
        Ok(())
    }

    /// Switches the family off: the settings and the list go. Nothing else in
    /// the database refers to them, thus no record, copy or state changes.
    pub async fn disable(&self) -> Result<()> {
        let mut tx = self.0.pool().begin().await?;
        sqlx::query("DELETE FROM steam_family_app")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM steam_family")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Replaces the whole list with the list that just came in.
    ///
    /// It replaces, it does not accumulate, and the old rows are really
    /// deleted: the migration `0011_steam_family` gives the reason. The
    /// `igdb_id` of an appid that stays is kept, because the identity pass
    /// writes it and the synchronisation does not ask IGDB again.
    ///
    /// It gives `false` and writes nothing if the family was switched off
    /// while the synchronisation asked Steam: a list must not come back that
    /// the user just removed.
    pub async fn replace(
        &self,
        group: &FamilyGroup,
        apps: &[FamilyApp],
        now: OffsetDateTime,
    ) -> Result<bool> {
        let now = now.unix_timestamp();
        let mut tx = self.0.pool().begin().await?;

        let on = sqlx::query("SELECT 1 FROM steam_family WHERE id = 1")
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
        if !on {
            return Ok(false);
        }

        let known: HashMap<String, i64> =
            sqlx::query("SELECT app_id, igdb_id FROM steam_family_app WHERE igdb_id IS NOT NULL")
                .fetch_all(&mut *tx)
                .await?
                .iter()
                .map(|row| (row.get("app_id"), row.get("igdb_id")))
                .collect();

        sqlx::query("DELETE FROM steam_family_app")
            .execute(&mut *tx)
            .await?;

        for app in apps {
            let owners = serde_json::to_string(&app.owners).map_err(|_| StorageError::Corrupt {
                column: "owners",
                value: format!("{:?}", app.owners),
            })?;
            sqlx::query(
                "INSERT INTO steam_family_app (app_id, title, owners, igdb_id, captured_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (app_id) DO NOTHING",
            )
            .bind(&app.app_id)
            .bind(&app.title)
            .bind(owners)
            .bind(app.igdb_id.or_else(|| known.get(&app.app_id).copied()))
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        let group_id = match group {
            FamilyGroup::Member(id) => Some(id.as_str()),
            FamilyGroup::NotMember => None,
        };
        sqlx::query(
            "UPDATE steam_family SET family_groupid = ?, refreshed_at = ?, last_error = NULL
             WHERE id = 1",
        )
        .bind(group_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Writes why the stage did not end. The list of the last stage that ended
    /// stays, and so do its marks.
    pub async fn record_error(&self, reason: &str) -> Result<()> {
        sqlx::query("UPDATE steam_family SET last_error = ? WHERE id = 1")
            .bind(reason)
            .execute(self.0.pool())
            .await?;
        Ok(())
    }

    /// The list of the last stage that ended, in the order of the titles.
    pub async fn apps(&self) -> Result<Vec<FamilyApp>> {
        sqlx::query(
            "SELECT app_id, title, owners, igdb_id FROM steam_family_app
              ORDER BY title, app_id",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(app_from_row)
        .collect()
    }

    /// The appids that the identity pass has not joined to IGDB yet.
    pub async fn missing_igdb(&self) -> Result<Vec<String>> {
        Ok(
            sqlx::query(
                "SELECT app_id FROM steam_family_app WHERE igdb_id IS NULL ORDER BY app_id",
            )
            .fetch_all(self.0.pool())
            .await?
            .iter()
            .map(|row| row.get("app_id"))
            .collect(),
        )
    }

    /// Writes the IGDB record of each appid that IGDB knows.
    pub async fn set_igdb_ids(&self, joins: &HashMap<String, i64>) -> Result<()> {
        let mut tx = self.0.pool().begin().await?;
        for (app_id, igdb_id) in joins {
            sqlx::query("UPDATE steam_family_app SET igdb_id = ? WHERE app_id = ?")
                .bind(igdb_id)
                .bind(app_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

/// One row of `steam_family_app`. The library reads the same columns.
pub(crate) fn app_from_row(row: &SqliteRow) -> Result<FamilyApp> {
    let owners_json: String = row.get("owners");
    let owners: Vec<FamilyOwner> =
        serde_json::from_str(&owners_json).map_err(|_| StorageError::Corrupt {
            column: "owners",
            value: owners_json,
        })?;
    Ok(FamilyApp {
        app_id: row.get("app_id"),
        title: row.get("title"),
        owners,
        igdb_id: row.get("igdb_id"),
    })
}
