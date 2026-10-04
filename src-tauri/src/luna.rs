//! The Luna use case: to keep the catalogue that Prime includes.
//!
//! It is apart from the synchronisation, as the prices are, and for the same
//! reason: Luna is a private endpoint of Amazon that can change any day, and a
//! Luna that is down must not prevent the synchronisation of Steam.
//!
//! It writes in `luna_region` and `luna_catalog` and nowhere else. The match
//! with the records happens when the library is read, in
//! `storage::repositories::library`, thus nothing here touches `game`,
//! `store_entry` or `user_state`.
//!
//! It takes its collaborators and does not get them from the global state, as
//! the prices do: thus you can test it from end to end against a pretend server
//! and a real database, with no start of Tauri.

use metadata::LunaClient;
use metadata::luna::LunaRegion;
use serde::Serialize;
use storage::Database;
use storage::repositories::LunaRepository;
use time::OffsetDateTime;

use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct LunaReport {
    /// How many games the catalogue has now.
    pub games: usize,
    /// The country that Luna says the catalogue is for.
    pub territory: Option<String>,
}

/// Switches Luna on for a country, or changes the country.
///
/// It asks for the catalogue **before** it keeps anything, as the ITAD key is
/// examined before it is kept: a country that does not answer must not leave
/// Luna switched on with nothing to show.
pub async fn enable(
    db: &Database,
    client: &LunaClient,
    country: &str,
) -> Result<LunaReport, AppError> {
    let region = LunaRegion::find(country).ok_or_else(|| {
        AppError::Message(format!(
            "Luna does not operate in {country}, or nobody examined it: select one of the countries of the list"
        ))
    })?;
    let catalog = fetch(client, &region).await?;

    let luna = LunaRepository(db);
    luna.set_country(region.country, region.domain).await?;
    luna.replace_catalog(&catalog, OffsetDateTime::now_utc())
        .await?;
    Ok(LunaReport {
        games: catalog.offers.len(),
        territory: catalog.territory,
    })
}

/// Asks for the catalogue again and replaces the one that is kept.
///
/// A failure of Luna keeps the catalogue of the last refresh: the marks of the
/// library stay as they were, and the error says why. A failure of the
/// database goes up as it is.
pub async fn refresh(db: &Database, client: &LunaClient) -> Result<LunaReport, AppError> {
    let luna = LunaRepository(db);
    let settings = luna.settings().await?.ok_or_else(|| {
        AppError::Message("Luna is switched off: select your country in Utilities".to_owned())
    })?;
    let region = LunaRegion::find(&settings.country).ok_or_else(|| {
        AppError::Message(format!(
            "Luna is no longer offered for {}: select your country again",
            settings.country
        ))
    })?;

    let catalog = fetch(client, &region).await?;
    luna.replace_catalog(&catalog, OffsetDateTime::now_utc())
        .await?;
    Ok(LunaReport {
        games: catalog.offers.len(),
        territory: catalog.territory,
    })
}

/// Switches Luna off. The marks go with the catalogue; nothing else changes.
pub async fn disable(db: &Database) -> Result<(), AppError> {
    Ok(LunaRepository(db).clear().await?)
}

/// The catalogue, or an error if it is empty.
///
/// An answer `200` with no game is not a Luna with no games: it is a page whose
/// shape changed. To keep it would delete every mark of the library on the day
/// that Amazon moves a field.
async fn fetch(client: &LunaClient, region: &LunaRegion) -> Result<domain::LunaCatalog, AppError> {
    let catalog = client.prime_catalog(region).await?;
    if catalog.offers.is_empty() {
        return Err(AppError::Message(
            "Luna gave a catalogue with no games: its page probably changed, and the last catalogue stays".to_owned(),
        ));
    }
    Ok(catalog)
}
