//! The identity use case: to turn store entries into game records.
//!
//! The order is not negotiable. The external identifier is first, because it is
//! exact; the similarity of titles applies only when there is no identifier, and
//! there `domain::matching` decides and sends to review when there is doubt.
//!
//! The three stores have an external identifier, and each one has its own: the
//! Steam appid, the `external_id` of Galaxy and the offer of Epic. The joins are
//! requested **all together before the loop**, in batches of 500, and not one
//! copy at a time. At 4 requests each second, a library of 1,200 copies took five
//! minutes to join and the user cancelled before the end; the copies that did not
//! join fell to the search by title, which is the method with doubt that the
//! identifier exists to prevent.
//!
//! It writes in `game`, `game_link` and `match_candidate`. Never in `store_entry`
//! — that belongs to the store — and never in `user_state` — that belongs to the
//! user.

use std::collections::HashMap;

use domain::{
    Game, GameId, GameLink, LinkMethod, MatchDecision, StoreEntry, StoreEntryId, StoreId, matching,
};
use metadata::IgdbClient;
use metadata::igdb::{ExternalSource, IgdbCredentials, IgdbToken};
use serde::Serialize;
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, MatchCandidateRepository, StoreEntryRepository,
};

use crate::error::AppError;
use crate::sync::{ProgressSink, SyncProgress};

#[derive(Debug, Default, Serialize)]
pub struct IdentityReport {
    /// The entries linked with no question.
    pub linked: usize,
    /// The entries sent to the review queue.
    pub review: usize,
    /// The entries with no candidate: not even IGDB knows them.
    pub unknown: usize,
    /// The user stopped in the middle. The matches stay: the operation is
    /// idempotent.
    pub cancelled: bool,
    /// The provider stopped the requests and the pass stopped there, with the
    /// reason.
    ///
    /// This is a result, not an error: the matches made to that point are kept
    /// and the next pass continues from there. Only a condition that prevents
    /// all progress goes up as an error, and that is a failure of the
    /// database.
    pub stopped: Option<String>,
}

/// How many games the pass matches before it keeps the result.
///
/// Before, the pass wrote one time only, at the end. With one thousand games
/// that is minutes — IGDB accepts four requests each second — and a 429 at game
/// three hundred lost all of the pass: no link written, and the user had to
/// start again. Twenty-five games is approximately ten seconds of work, and that
/// is what you can lose now.
///
/// An extra write breaks nothing: `rebuild_auto` writes the same set of links
/// each time, thus twenty calls give the same result as one call.
const BATCH: usize = 25;

pub async fn resolve(
    db: &Database,
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    progress: &dyn ProgressSink,
) -> Result<IdentityReport, AppError> {
    let entries = StoreEntryRepository(db);
    // The entries that never had a record and the entries that have a record
    // made only with the title of the store: you already see the second group in
    // the library, but they still wait for a true identity.
    let mut pending = entries.unlinked().await?;
    pending.extend(entries.pending_metadata().await?);

    let mut report = IdentityReport::default();
    let mut links = GameLinkRepository(db).all().await?;
    let total = pending.len();
    let mut since_last_save = 0;

    progress.report(SyncProgress {
        store: "igdb".to_owned(),
        stage: "joining identifiers",
        done: 0,
        total,
    });
    // The join goes before the loop, thus a stop here leaves nothing
    // incomplete: it is batch zero. The pass says why it stops and matches
    // nothing, and does not let all of the library fall to the search by title,
    // which links worse than the identifier links.
    let external = match external_ids(igdb, credentials, token, &pending).await {
        Ok(external) => external,
        Err(AppError::Metadata(error)) => {
            report.stopped = Some(error.to_string());
            return Ok(report);
        }
        Err(other) => return Err(other),
    };

    for (index, entry) in pending.into_iter().enumerate() {
        // It stops between games, never in the middle of one. The decisions
        // already made stay and the next pass continues from there.
        if progress.cancelled() {
            report.cancelled = true;
            break;
        }
        progress.report(SyncProgress {
            store: entry.store.as_str().to_owned(),
            stage: "matching",
            done: index,
            total,
        });

        let decision = match decide(
            igdb,
            credentials,
            token,
            &entry,
            external.get(&entry.id).copied(),
        )
        .await
        {
            Ok(decision) => decision,
            // A stop from the provider stops the pass at this point, and the
            // earlier work is still kept. A failure of the database goes up: if
            // you cannot write, there is nothing to keep.
            Err(AppError::Metadata(error)) => {
                report.stopped = Some(error.to_string());
                break;
            }
            Err(other) => return Err(other),
        };
        let local_record = links
            .iter()
            .find(|link| link.store_entry_id == entry.id)
            .map(|link| link.game_id);

        match decision {
            MatchDecision::Auto {
                igdb_id,
                confidence,
            } => {
                let game_id =
                    match ensure_game(db, igdb, credentials, token, igdb_id, &entry, local_record)
                        .await
                    {
                        Ok(game_id) => game_id,
                        Err(AppError::Metadata(error)) => {
                            report.stopped = Some(error.to_string());
                            break;
                        }
                        Err(other) => return Err(other),
                    };
                // The entry can already carry a local link: the new link
                // replaces it and does not accumulate. With two proposals for
                // the same entry, the unique index would decide the winner by
                // the order of insertion.
                links.retain(|link| link.store_entry_id != entry.id);
                links.push(GameLink {
                    game_id,
                    store_entry_id: entry.id,
                    confidence,
                    method: LinkMethod::Auto,
                });
                MatchCandidateRepository(db).clear(entry.id).await?;
                report.linked += 1;
            }
            // With no decision, the local link stays as it was: it is already in
            // `links` and `rebuild_auto` will write it again. To remove it would
            // make a game that the user already saw go out of the library.
            MatchDecision::Review { candidates } => {
                if candidates.is_empty() {
                    report.unknown += 1;
                } else {
                    report.review += 1;
                }
                MatchCandidateRepository(db)
                    .replace(entry.id, &candidates)
                    .await?;
            }
        }

        since_last_save += 1;
        if since_last_save == BATCH {
            GameLinkRepository(db).rebuild_auto(&links).await?;
            since_last_save = 0;
        }
    }

    // And the last batch, which almost never ends exactly at the limit.
    // `rebuild_auto` writes the automatic links in one operation and keeps the
    // manual links, which is the guarantee of phase 2.
    GameLinkRepository(db).rebuild_auto(&links).await?;
    GameRepository(db).soft_delete_orphans().await?;
    Ok(report)
}

/// The matching with no IGDB: it groups the copies by normalised title and makes
/// a record for them with what the store says.
///
/// It exists because to block all of the application until the user gets Twitch
/// credentials is very hard at the first start. What comes out of here is a true
/// library — with its status and its store badges — that waits for metadata, and
/// the same title in two stores already falls into one record: the normalisation
/// is sufficient for that, and IGDB only adds the certainty.
pub async fn resolve_local(
    db: &Database,
    progress: &dyn ProgressSink,
) -> Result<IdentityReport, AppError> {
    let games = GameRepository(db);
    let mut report = IdentityReport::default();
    let mut links = GameLinkRepository(db).all().await?;

    let pending = StoreEntryRepository(db).unlinked().await?;
    let total = pending.len();

    for (index, entry) in pending.into_iter().enumerate() {
        if progress.cancelled() {
            report.cancelled = true;
            break;
        }
        progress.report(SyncProgress {
            store: entry.store.as_str().to_owned(),
            stage: "grouping by title",
            done: index,
            total,
        });

        let sort_title = matching::normalize(&entry.title);
        let game_id = match games.find_local_by_sort_title(&sort_title).await? {
            Some(existing) => existing.id,
            None => {
                let game = local_game(&entry);
                games.upsert(&game).await?;
                game.id
            }
        };

        links.retain(|link| link.store_entry_id != entry.id);
        links.push(GameLink {
            game_id,
            store_entry_id: entry.id,
            confidence: matching::LOCAL_TITLE_CONFIDENCE,
            method: LinkMethod::Auto,
        });
        report.linked += 1;
    }

    GameLinkRepository(db).rebuild_auto(&links).await?;
    Ok(report)
}

/// Joins against `external_games` each entry that carries an identifier, one
/// store at a time and in batches.
///
/// The entries that do not join do not appear in the map, and that is usual: the
/// Amazon keys that GOG gives, the sound tracks and the prologues have no record
/// in IGDB, and they are most of what fails. Those copies continue on the path
/// of the title.
async fn external_ids(
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    pending: &[StoreEntry],
) -> Result<HashMap<StoreEntryId, i64>, AppError> {
    const SOURCES: [(StoreId, ExternalSource); 3] = [
        (StoreId::Steam, ExternalSource::Steam),
        (StoreId::Gog, ExternalSource::Gog),
        (StoreId::Epic, ExternalSource::Epic),
    ];

    let mut resolved = HashMap::new();

    for (store, source) in SOURCES {
        let from_store: Vec<(StoreEntryId, String)> = pending
            .iter()
            .filter(|entry| entry.store == store)
            .filter_map(|entry| external_uid(entry).map(|uid| (entry.id, uid)))
            .collect();
        if from_store.is_empty() {
            continue;
        }

        // The same game can be in two accounts of the same store, and to ask
        // twice for it would use space of the batch.
        let mut uids: Vec<String> = from_store.iter().map(|(_, uid)| uid.clone()).collect();
        uids.sort_unstable();
        uids.dedup();

        let joins = igdb
            .by_external_ids(credentials, token, source, &uids)
            .await?;
        for (id, uid) in from_store {
            if let Some(igdb_id) = joins.get(&uid) {
                resolved.insert(id, *igdb_id);
            }
        }
    }

    Ok(resolved)
}

/// The identifier with which each store appears in `external_games`.
///
/// Steam and GOG publish their identifier in the copy itself. Epic does not:
/// IGDB indexes the **offer** of its store, which does not go in the asset of
/// the launcher, thus the connector resolves the offer during the
/// synchronisation and puts it in `raw`. An Epic copy synchronised before that
/// field existed does not have it, and it matches by title until the next
/// pass.
fn external_uid(entry: &StoreEntry) -> Option<String> {
    match entry.store {
        StoreId::Steam | StoreId::Gog => Some(entry.store_app_id.clone()),
        StoreId::Epic => entry
            .raw
            .get("offerId")
            .and_then(|offer| offer.as_str())
            .map(str::to_owned),
    }
}

async fn decide(
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    entry: &StoreEntry,
    external: Option<i64>,
) -> Result<MatchDecision, AppError> {
    // The identifier of the store is exact and removes all of the uncertainty
    // of the similarity of titles.
    if let Some(igdb_id) = external {
        return Ok(matching::decide_by_external_id(igdb_id));
    }

    let candidates = igdb.search(credentials, token, &entry.title).await?;
    Ok(matching::decide_by_title(&entry.title, None, &candidates))
}

/// Creates the record if it does not exist. The `game` table is also the cache
/// of IGDB: if the game is already there, the code never asks again.
///
/// `local_record` is the record with no metadata to which this copy was already
/// attached, if there was one. The code **uses its identifier again** and does
/// not create a new record, and that is all of the difference: `user_state` is
/// attached to the `game_id`, thus a new record would leave the status that the
/// user wrote with no owner.
async fn ensure_game(
    db: &Database,
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    igdb_id: i64,
    entry: &StoreEntry,
    local_record: Option<GameId>,
) -> Result<GameId, AppError> {
    let games = GameRepository(db);
    if let Some(existing) = games.find_by_igdb(igdb_id).await? {
        return Ok(existing.id);
    }

    // With no earlier record the code creates one; with a record it writes the
    // record that existed again. `GameId::default()` is `GameId::new()`, with a
    // new UUIDv7.
    let id = local_record.unwrap_or_default();
    let fetched = igdb.game(credentials, token, igdb_id).await?;
    let game = match fetched {
        Some(meta) => Game {
            id,
            canonical_title: meta.name.clone(),
            sort_title: matching::normalize(&meta.name),
            igdb_id: Some(meta.igdb_id),
            cover_url: meta.cover_url,
            summary: meta.summary,
            released_at: meta.released_at,
            genres: meta.genres,
        },
        // IGDB knows the identifier but does not give back the record: a record
        // with the title of the store is better than no record.
        None => Game {
            id,
            ..local_game(entry)
        },
    };

    games.upsert(&game).await?;
    Ok(game.id)
}

/// A record with no metadata, built with what the store says. This is what the
/// code creates when the user declares that a game is not in IGDB.
pub fn local_game(entry: &StoreEntry) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: entry.title.clone(),
        sort_title: matching::normalize(&entry.title),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    }
}
