use domain::{LunaCatalog, LunaOffer};
use serde::Serialize;
use sqlx::Row;
use time::OffsetDateTime;

use crate::{Database, Result};

/// What the user selected for Luna, and when the catalogue was last asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LunaSettings {
    pub country: String,
    /// The domain of the Luna web page for that country, such as
    /// `luna.amazon.es`.
    pub site: String,
    /// The country that the last answer of Luna gave.
    pub territory: Option<String>,
    /// Seconds from the epoch. `None` until the first refresh succeeds.
    pub refreshed_at: Option<i64>,
}

pub struct LunaRepository<'a>(pub &'a Database);

impl LunaRepository<'_> {
    /// The settings, or `None` if Luna is switched off.
    pub async fn settings(&self) -> Result<Option<LunaSettings>> {
        Ok(sqlx::query(
            "SELECT country, site, territory, refreshed_at FROM luna_region WHERE id = 1",
        )
        .fetch_optional(self.0.pool())
        .await?
        .map(|row| LunaSettings {
            country: row.get("country"),
            site: row.get("site"),
            territory: row.get("territory"),
            refreshed_at: row.get("refreshed_at"),
        }))
    }

    /// Switches Luna on for a country, or changes the country.
    ///
    /// The catalogue stays: Amazon selects it from the connection and not from
    /// the country, thus a change of country changes only the links.
    pub async fn set_country(&self, country: &str, site: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO luna_region (id, country, site) VALUES (1, ?, ?)
             ON CONFLICT (id) DO UPDATE SET country = excluded.country, site = excluded.site",
        )
        .bind(country)
        .bind(site)
        .execute(self.0.pool())
        .await?;
        Ok(())
    }

    /// Replaces the whole catalogue with the catalogue that just came in.
    ///
    /// It replaces, it does not accumulate, and the old rows are really
    /// deleted: the migration `0010_luna` gives the reason. The delete, the
    /// inserts and the date go in one transaction, thus a failure in the middle
    /// leaves the catalogue of the last refresh and not one half of each.
    pub async fn replace_catalog(&self, catalog: &LunaCatalog, now: OffsetDateTime) -> Result<()> {
        let now = now.unix_timestamp();
        let mut tx = self.0.pool().begin().await?;

        sqlx::query("DELETE FROM luna_catalog")
            .execute(&mut *tx)
            .await?;

        for offer in &catalog.offers {
            sqlx::query(
                "INSERT INTO luna_catalog (product_id, title, path, asin, captured_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (product_id) DO NOTHING",
            )
            .bind(&offer.product_id)
            .bind(&offer.title)
            .bind(&offer.path)
            .bind(&offer.asin)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query("UPDATE luna_region SET territory = ?, refreshed_at = ? WHERE id = 1")
            .bind(&catalog.territory)
            .bind(now)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// The catalogue of the last refresh, in the order of the titles.
    pub async fn catalog(&self) -> Result<Vec<LunaOffer>> {
        Ok(sqlx::query(
            "SELECT product_id, title, path, asin FROM luna_catalog ORDER BY title, product_id",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(|row| LunaOffer {
            product_id: row.get("product_id"),
            title: row.get("title"),
            path: row.get("path"),
            asin: row.get("asin"),
        })
        .collect())
    }

    /// Switches Luna off: the settings and the catalogue go. Nothing else in
    /// the database refers to them, thus no record, copy or state changes.
    pub async fn clear(&self) -> Result<()> {
        let mut tx = self.0.pool().begin().await?;
        sqlx::query("DELETE FROM luna_catalog")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM luna_region")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
