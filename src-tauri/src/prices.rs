//! The price use case: to give a price to each game of the wishlist.
//!
//! It is apart from the synchronisation and not inside it, and that is
//! deliberate. They are different things: a synchronisation reads the stores of
//! the user, and this asks a third party what something costs. To put them
//! together would let an ITAD that is down prevent the synchronisation of Steam,
//! which is exactly what phase 7 prevented.
//!
//! It writes in `price_snapshot`, in `price_low` and in the two columns of
//! `game` that keep the ITAD identifier. Never in `store_entry` — that belongs
//! to the store — and never in `user_state` — that belongs to the user.
//!
//! It takes its collaborators and does not get them from the global state, as
//! the synchronisation does: thus you can test it from end to end against a
//! pretend server and a real database, with no start of Tauri.

use std::collections::HashMap;

use domain::GameId;
use metadata::ItadClient;
use metadata::itad::ItadCredentials;
use serde::Serialize;
use storage::Database;
use storage::repositories::{GameRepository, PriceRepository, PriceTarget};

use crate::error::AppError;
use crate::sync::{ProgressSink, SyncProgress};

/// Where the prices come from. It is the name that the interface shows, and also
/// the name that goes in the progress.
const PROVIDER: &str = "itad";

#[derive(Debug, Default, Serialize)]
pub struct PriceReport {
    /// The wished-for games with one or more stores that sell them at this
    /// moment.
    pub priced: usize,
    /// The wished-for games that ITAD cannot identify. This is not an error:
    /// there are games that it does not have, and the next pass asks for them
    /// again.
    pub unknown: usize,
    /// The user stopped in the middle. The data received stays: the operation is
    /// idempotent.
    pub cancelled: bool,
}

pub async fn refresh(
    db: &Database,
    itad: &ItadClient,
    credentials: &ItadCredentials,
    progress: &dyn ProgressSink,
) -> Result<PriceReport, AppError> {
    let prices = PriceRepository(db);
    let targets = prices.targets().await?;
    let mut report = PriceReport::default();

    // A game that is no longer in a wishlist stops having a price. This is the
    // first operation and it uses the complete list — not only the games that
    // the pass asks about — thus a cancel in the middle deletes nothing that is
    // still applicable.
    let live: Vec<GameId> = targets.iter().map(|target| target.game_id).collect();
    prices.forget_missing(&live).await?;

    // One identifier can apply to more than one record: two local records of the
    // same game, not yet unified, resolve to the same ITAD game, and the two
    // must get their price.
    let mut by_id: HashMap<String, Vec<GameId>> = HashMap::new();
    let total = targets.len();

    for (index, target) in targets.into_iter().enumerate() {
        // It stops between games, never in the middle of one.
        if progress.cancelled() {
            report.cancelled = true;
            break;
        }
        progress.report(SyncProgress {
            store: PROVIDER.to_owned(),
            stage: "searching in ITAD",
            done: index,
            total,
        });

        match resolve(db, itad, credentials, &target).await? {
            Some(itad_id) => by_id.entry(itad_id).or_default().push(target.game_id),
            None => report.unknown += 1,
        }
    }

    if by_id.is_empty() {
        return Ok(report);
    }

    progress.report(SyncProgress {
        store: PROVIDER.to_owned(),
        stage: "prices",
        done: 0,
        total: by_id.len(),
    });

    // One query for each two hundred games: the client divides the list.
    let ids: Vec<String> = by_id.keys().cloned().collect();
    for game_prices in itad.prices(credentials, &ids).await? {
        let Some(games) = by_id.get(&game_prices.provider_id) else {
            continue;
        };
        for game_id in games {
            prices.save(*game_id, &game_prices).await?;
            if !game_prices.deals.is_empty() {
                report.priced += 1;
            }
        }
    }

    Ok(report)
}

/// The identifier with which ITAD knows this game.
///
/// The identifier already known, if there is one. If there is not, the Steam
/// appid, which is exact, and last the title, which is a guess. The result is
/// written in the record: a wishlist is short, but to ask the same question at
/// each pass uses quota for nothing.
async fn resolve(
    db: &Database,
    itad: &ItadClient,
    credentials: &ItadCredentials,
    target: &PriceTarget,
) -> Result<Option<String>, AppError> {
    if let Some(known) = &target.itad_id {
        return Ok(Some(known.clone()));
    }

    let found = match &target.steam_app_id {
        Some(app_id) => itad.lookup_by_steam_app_id(credentials, app_id).await?,
        None => itad.lookup_by_title(credentials, &target.title).await?,
    };

    match found {
        Some(game) => {
            GameRepository(db)
                .set_itad(target.game_id, &game.id, &game.slug)
                .await?;
            Ok(Some(game.id))
        }
        None => Ok(None),
    }
}
