//! The commands given to the interface. They control the use cases and
//! translate types: the logic lives in the domain and adapter crates.

// Public because `generate_handler!` must reach the item that
// `#[tauri::command]` generates, and a `pub use` of the function alone does not
// bring it.
pub mod epic;
pub mod gog;

use domain::{
    AuthContext, ConnectorState, EntryKind, GameId, GameLink, LinkMethod, PlayStatus,
    ScoredCandidate, StoreAccount, StoreAccountId, StoreEntryId, StoreId, UserState,
};
use metadata::igdb::{IgdbCredentials, IgdbToken};
use metadata::itad::ItadCredentials;
use serde::{Deserialize, Serialize};
use storage::repositories::{
    ConnectorStateRepository, GameLinkRepository, GameRepository, LibraryRepository, LibraryRow,
    MatchCandidateRepository, PriceRepository, PriceRow, StoreAccountRepository,
    StoreEntryRepository, UserStateRepository,
};
use tauri::{AppHandle, Emitter, State};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;
use crate::identity::{self, IdentityReport};
use crate::prices::{self, PriceReport};
use crate::state::{AppState, IGDB_CREDENTIALS, IGDB_TOKEN, ITAD_CREDENTIALS, credential_key};
use crate::sync::{self, ProgressSink, SyncProgress, SyncReport};

#[derive(Serialize)]
pub struct AppInfo {
    pub version: &'static str,
    /// `keyring` or `passphrase`: it decides whether the interface must ask for
    /// a passphrase.
    pub secrets_backend: secrets::Backend,
    pub unlocked: bool,
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> Result<AppInfo, AppError> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        secrets_backend: state.backend,
        unlocked: state.is_unlocked().await,
    })
}

/// Opens the encrypted store on the machines with no keyring.
#[tauri::command]
pub async fn unlock_secrets(
    state: State<'_, AppState>,
    passphrase: String,
) -> Result<(), AppError> {
    state.unlock(&passphrase).await?;
    Ok(())
}

/// Connects a Steam account and examines the key against the API before it keeps
/// the key: thus the user sees a copy-and-paste error immediately and not as an
/// empty library.
#[tauri::command]
pub async fn connect_steam(
    state: State<'_, AppState>,
    api_key: String,
    steam_id: String,
) -> Result<StoreAccountId, AppError> {
    let connector = state
        .connectors
        .get(&StoreId::Steam)
        .ok_or_else(|| AppError::Message("there is no Steam connector".to_owned()))?;

    let session = connector
        .authenticate(&AuthContext::ApiKey {
            key: api_key.trim().to_owned(),
            account_ref: steam_id.trim().to_owned(),
        })
        .await?;

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: session.account_ref.clone(),
        display_name: session.display_name.clone(),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(&state.db).upsert(&account).await?;

    // The credential goes to the store of secrets. The database only knows that
    // the account exists.
    state
        .secrets()
        .await?
        .set(&credential_key(&account), &session.credential)?;

    Ok(id)
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<AccountView>, AppError> {
    Ok(StoreAccountRepository(&state.db)
        .active()
        .await?
        .into_iter()
        .map(|account| AccountView {
            store: account.store.as_str(),
            account_ref: account.account_ref,
            display_name: account.display_name,
            last_sync_at: account.last_sync_at.map(|t| t.unix_timestamp()),
        })
        .collect())
}

/// Disconnects one account and removes its credential.
///
/// The database operation is logical: the store entries remain available as
/// history, and the records and the state written by the user remain available
/// in the library. The secret is removed only after the database transaction
/// succeeds, so a failed transaction never leaves a connected account without
/// its credential.
#[tauri::command]
pub async fn disconnect_account(
    state: State<'_, AppState>,
    store: StoreId,
    account_ref: String,
) -> Result<(), AppError> {
    let secrets = state.secrets().await?;
    disconnect_account_for(&state.db, secrets.as_ref(), store, &account_ref).await
}

/// The disconnect use case without Tauri state. Integration tests use this
/// entry point to exercise the database and the secret store together.
pub async fn disconnect_account_for(
    db: &storage::Database,
    secrets: &dyn secrets::SecretStore,
    store: StoreId,
    account_ref: &str,
) -> Result<(), AppError> {
    let account = StoreAccountRepository(db)
        .active()
        .await?
        .into_iter()
        .find(|account| account.store == store && account.account_ref == account_ref)
        .ok_or_else(|| {
            AppError::Message(format!(
                "there is no connected {} account with that reference",
                store.as_str()
            ))
        })?;

    StoreAccountRepository(db).soft_delete(account.id).await?;
    secrets.delete(&credential_key(&account))?;
    Ok(())
}

#[derive(Serialize)]
pub struct AccountView {
    pub store: &'static str,
    pub account_ref: String,
    pub display_name: Option<String>,
    pub last_sync_at: Option<i64>,
}

/// The connectors that are switched off or that failed the last time they ran.
///
/// Only those: a store with nothing to say has no row, and the interface reads
/// the absence as on and healthy. Writing the healthy state down would mean
/// deciding on the first run which stores exist.
#[tauri::command]
pub async fn connector_states(state: State<'_, AppState>) -> Result<Vec<ConnectorState>, AppError> {
    Ok(ConnectorStateRepository(&state.db).all().await?)
}

/// Turns a connector off, or back on.
///
/// It is what makes a broken store survivable: Epic rests on the private API of
/// its own launcher, and the day that changes the user can switch it off and
/// keep the rest of the library working instead of watching every
/// synchronisation fail.
#[tauri::command]
pub async fn set_connector_enabled(
    state: State<'_, AppState>,
    store: StoreId,
    enabled: bool,
) -> Result<(), AppError> {
    ConnectorStateRepository(&state.db)
        .set_enabled(store, enabled)
        .await?;
    Ok(())
}

/// Sends the progress to the window and reads the cancel flag.
struct WindowProgress<'a> {
    app: AppHandle,
    state: &'a AppState,
}

impl ProgressSink for WindowProgress<'_> {
    fn report(&self, progress: SyncProgress) {
        // If the window is no longer there, the progress does not matter: that
        // is not a reason to stop an operation that continues correctly.
        let _ = self.app.emit("sync:progress", progress);
    }

    fn cancelled(&self) -> bool {
        self.state.operation_cancelled()
    }
}

/// The synchronisation runs in the Tauri runtime and sends progress, thus the
/// window continues to answer while it runs.
#[tauri::command]
pub async fn sync_now(app: AppHandle, state: State<'_, AppState>) -> Result<SyncReport, AppError> {
    // The guard lives to the end of the command, and it is what clears the
    // cancel flag when the command goes away.
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    let progress = WindowProgress {
        app: app.clone(),
        state: &state,
    };
    sync::sync_all(&state, &progress).await
}

/// This applies to the three long operations: all of them stop at the next safe
/// point, and only one of them runs, thus this cancel reaches the operation that
/// the user sees.
#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>) {
    state.cancel_operation();
}

/// All of the library in one query. With one thousand games, one query for each
/// game to find its stores is what makes the grid jump.
#[tauri::command]
pub async fn library(state: State<'_, AppState>) -> Result<Vec<LibraryRow>, AppError> {
    Ok(LibraryRepository(&state.db).all().await?)
}

/// One save that the interface asks for.
///
/// The command that saves one takes the same four fields apart, because that is
/// what an `invoke` with named arguments gives; the batch takes a list of these.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateUpdate {
    pub game_id: String,
    pub status: Option<PlayStatus>,
    pub rating: Option<u8>,
    pub notes: Option<String>,
}

/// The only data that the user writes. No later synchronisation touches it.
///
/// It gives back the row that it wrote, and that is the whole point: before,
/// the interface answered a save with a complete refresh — all of the library,
/// all of the review queue and all of the prices, eight commands — to see one
/// status change on one row.
#[tauri::command]
pub async fn set_user_state(
    state: State<'_, AppState>,
    game_id: String,
    status: Option<PlayStatus>,
    rating: Option<u8>,
    notes: Option<String>,
) -> Result<LibraryRow, AppError> {
    let update = StateUpdate {
        game_id,
        status,
        rating,
        notes,
    };
    save_states(&state.db, &[update])
        .await?
        .pop()
        .ok_or_else(|| AppError::Message("that game is no longer in the library".to_owned()))
}

/// The same save over more than one game, in one call and in one transaction.
///
/// The bulk bar marked thirty games with thirty commands, one after another,
/// and then asked for the library again. Here it is one command, and the answer
/// is exactly the rows that changed.
#[tauri::command]
pub async fn set_user_state_many(
    state: State<'_, AppState>,
    updates: Vec<StateUpdate>,
) -> Result<Vec<LibraryRow>, AppError> {
    save_states(&state.db, &updates).await
}

/// The body that "save one" and "save many" share. Because it is the same body,
/// the batch takes no short cut against the one-at-a-time path: the same dates
/// are kept, the same transaction writes, and the rows come back from the same
/// query as the list.
///
/// It takes a `Database` and not the state so that a test reaches it with no
/// Tauri, in the same way as `summary`.
pub async fn save_states(
    db: &storage::Database,
    updates: &[StateUpdate],
) -> Result<Vec<LibraryRow>, AppError> {
    let states = UserStateRepository(db);
    let mut wanted = Vec::with_capacity(updates.len());

    for update in updates {
        let game_id = Uuid::parse_str(&update.game_id)
            .map(GameId::from_uuid)
            .map_err(|_| AppError::Message("invalid game identifier".to_owned()))?;

        // The two dates are not in the form, thus a save that does not know
        // them must give them back unchanged: the row is written again
        // complete, and without this a change of status would clear them.
        let previous = states.find(game_id).await?;
        wanted.push(UserState {
            game_id,
            status: update.status,
            rating: update.rating,
            notes: update.notes.clone(),
            started_at: previous.as_ref().and_then(|p| p.started_at),
            finished_at: previous.as_ref().and_then(|p| p.finished_at),
        });
    }

    states.save_many(&wanted).await?;

    // A record that the library no longer shows gives no row. It is not an
    // error here: the caller that saves one game turns the absence into one.
    let library = LibraryRepository(db);
    let mut rows = Vec::with_capacity(wanted.len());
    for state in &wanted {
        if let Some(row) = library.one(state.game_id).await? {
            rows.push(row);
        }
    }
    Ok(rows)
}

#[derive(Serialize)]
pub struct LibrarySummary {
    pub owned: i64,
    pub wishlist: i64,
    pub games: i64,
    pub pending_review: i64,
}

/// Four numbers, four counts.
///
/// Before, each number was the length of a list: the four lists together were
/// every row of `store_entry` and of `game`, with every `raw` JSON parsed, and
/// all of it was thrown away after the `len()`. The database counts, which is
/// what a database is for.
///
/// It takes a `Database` and not the state so that a test reaches it with no
/// Tauri, in the same way as `sync`, `prices` and `identity`.
pub async fn summary(db: &storage::Database) -> Result<LibrarySummary, AppError> {
    let entries = StoreEntryRepository(db);
    Ok(LibrarySummary {
        owned: entries.count_active(EntryKind::Owned).await?,
        wishlist: entries.count_active(EntryKind::Wishlist).await?,
        games: GameRepository(db).count_all().await?,
        pending_review: GameLinkRepository(db).unlinked_entry_count().await?,
    })
}

#[tauri::command]
pub async fn library_summary(state: State<'_, AppState>) -> Result<LibrarySummary, AppError> {
    summary(&state.db).await
}

/// Keeps the IGDB credentials of the user and examines them before: if the
/// client secret is incorrect, you know it here and not in the middle of the
/// first synchronisation.
#[tauri::command]
pub async fn set_igdb_credentials(
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> Result<(), AppError> {
    let credentials = IgdbCredentials {
        client_id: client_id.trim().to_owned(),
        client_secret: client_secret.trim().to_owned(),
    };
    let token = state.igdb.token(&credentials).await?;

    let secrets = state.secrets().await?;
    secrets.set(IGDB_CREDENTIALS, &serde_json::to_string(&credentials)?)?;
    secrets.set(IGDB_TOKEN, &serde_json::to_string(&token)?)?;
    Ok(())
}

#[tauri::command]
pub async fn has_igdb_credentials(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(state.secrets().await?.get(IGDB_CREDENTIALS)?.is_some())
}

/// A game that ITAD certainly knows. It is used to spend one query to test the
/// key before the application keeps it, in the same way as the Steam key is
/// tested against its API.
const ITAD_PROBE: &str = "620";

/// Keeps the ITAD key and the country of the user, and examines them before.
///
/// The country is not decoration: ITAD gives back the stores and the currency of
/// that market, thus a price request that does not say where you live gives the
/// price of a different place.
#[tauri::command]
pub async fn set_itad_credentials(
    state: State<'_, AppState>,
    key: String,
    country: String,
) -> Result<(), AppError> {
    let country = country.trim().to_uppercase();
    if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::Message(
            "the country must be a code of two letters, such as ES or DE".to_owned(),
        ));
    }

    let credentials = ItadCredentials {
        key: key.trim().to_owned(),
        country,
    };
    state
        .itad
        .lookup_by_steam_app_id(&credentials, ITAD_PROBE)
        .await?;

    state
        .secrets()
        .await?
        .set(ITAD_CREDENTIALS, &serde_json::to_string(&credentials)?)?;
    Ok(())
}

#[tauri::command]
pub async fn has_itad_credentials(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(state.secrets().await?.get(ITAD_CREDENTIALS)?.is_some())
}

/// Gives a price to the wishlist.
///
/// It has a button of its own and it is not a step of the synchronisation: a
/// question to a third party about what something costs cannot prevent the
/// synchronisation of the stores of the user.
#[tauri::command]
pub async fn refresh_prices(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PriceReport, AppError> {
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    let credentials = itad_credentials(&state).await?;
    let progress = WindowProgress {
        app: app.clone(),
        state: &state,
    };

    prices::refresh(&state.db, &state.itad, &credentials, &progress).await
}

/// The best price of each wished-for game, in one query.
#[tauri::command]
pub async fn prices(state: State<'_, AppState>) -> Result<Vec<PriceRow>, AppError> {
    Ok(PriceRepository(&state.db).all().await?)
}

async fn itad_credentials(state: &AppState) -> Result<ItadCredentials, AppError> {
    let raw = state
        .secrets()
        .await?
        .get(ITAD_CREDENTIALS)?
        .ok_or(AppError::MissingItadCredentials)?;
    Ok(serde_json::from_str(&raw)?)
}

/// Matches what came from the stores with the IGDB records.
///
/// With no IGDB credentials it does not stop: it groups the copies by title and
/// makes a record for them with what the store says. The library is usable from
/// the first start, and on the day that the user configures IGDB these records
/// get their metadata in place and lose nothing that the user wrote on them.
#[tauri::command]
pub async fn resolve_identities(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<IdentityReport, AppError> {
    let _guard = state.try_begin().ok_or(AppError::Busy)?;
    let progress = WindowProgress {
        app: app.clone(),
        state: &state,
    };

    // The matching is slow because of the limit of 4 requests each second of
    // IGDB, thus it reports each game and does not leave the window quiet for
    // several minutes.
    match igdb_session(&state).await {
        Ok((credentials, token)) => {
            identity::resolve(&state.db, &state.igdb, &credentials, &token, &progress).await
        }
        Err(AppError::MissingIgdbCredentials) => {
            identity::resolve_local(&state.db, &progress).await
        }
        Err(other) => Err(other),
    }
}

/// The Twitch token lasts approximately sixty days: it is kept and renewed only
/// when it really expires.
async fn igdb_session(state: &AppState) -> Result<(IgdbCredentials, IgdbToken), AppError> {
    let secrets = state.secrets().await?;
    let raw = secrets
        .get(IGDB_CREDENTIALS)?
        .ok_or(AppError::MissingIgdbCredentials)?;
    let credentials: IgdbCredentials = serde_json::from_str(&raw)?;

    let cached: Option<IgdbToken> = secrets
        .get(IGDB_TOKEN)?
        .and_then(|raw| serde_json::from_str(&raw).ok());

    if let Some(token) = cached
        && token.is_valid(OffsetDateTime::now_utc())
    {
        return Ok((credentials, token));
    }

    let token = state.igdb.token(&credentials).await?;
    secrets.set(IGDB_TOKEN, &serde_json::to_string(&token)?)?;
    Ok((credentials, token))
}

#[derive(Serialize)]
pub struct ReviewItem {
    pub store_entry_id: String,
    pub store: &'static str,
    pub title: String,
    /// What the store shows about this copy. It is the second half of the
    /// comparison: without it the user selects between IGDB candidates blindly
    /// and does not see what they compare them against.
    pub cover_url: Option<String>,
    pub store_url: Option<String>,
    pub candidates: Vec<ScoredCandidate>,
    /// The two best candidates have the same score.
    ///
    /// This is clearly the most common reason to come to this queue, and it does
    /// not mean that the matching has doubt between two different games: IGDB
    /// has duplicate records, and the editions of one game normalise to the same
    /// title. The flag exists so that the interface can group them and resolve
    /// them together and not one at a time.
    pub tie: bool,
}

/// When two candidates count as equal. It is the same margin with which the
/// domain refuses to decide, so that the queue groups exactly what the matching
/// refused because it was ambiguous.
fn is_tie(candidates: &[ScoredCandidate]) -> bool {
    match (candidates.first(), candidates.get(1)) {
        (Some(best), Some(second)) => {
            best.score - second.score < domain::matching::AMBIGUITY_MARGIN
        }
        _ => false,
    }
}

/// The review queue: what the automatic matching did not decide, with the
/// candidates that it found, so that the user can select without they search.
#[tauri::command]
pub async fn review_queue(state: State<'_, AppState>) -> Result<Vec<ReviewItem>, AppError> {
    let entries = StoreEntryRepository(&state.db).unlinked().await?;
    let candidates = MatchCandidateRepository(&state.db);

    let mut queue = Vec::with_capacity(entries.len());
    for entry in entries {
        let found = candidates.for_entry(entry.id).await?;
        queue.push(ReviewItem {
            store_entry_id: entry.id.as_uuid().to_string(),
            store: entry.store.as_str(),
            title: entry.title.clone(),
            cover_url: entry.cover_url.clone(),
            store_url: entry.store_url.clone(),
            tie: is_tie(&found),
            candidates: found,
        });
    }
    Ok(queue)
}

/// Confirms more than one match together.
///
/// It is not the automatic matching through the back door. The interface comes
/// with the best candidate already selected for the entries that are **not**
/// equal, because to repeat with a click what the screen already says is work
/// that nobody needs; but the interface shows that in a column and writes
/// nothing until the user confirms. The entries that are equal still come with
/// no selection, which is exactly what the threshold refused to decide. Each
/// pair becomes a `manual` link, which no algorithm will touch again. The only
/// work that this removes is the same action one hundred and fifty times.
#[tauri::command]
pub async fn review_confirm_many(
    state: State<'_, AppState>,
    decisions: Vec<(String, i64)>,
) -> Result<usize, AppError> {
    let mut done = 0;
    for (store_entry_id, igdb_id) in decisions {
        confirm_one(&state, &store_entry_id, igdb_id).await?;
        done += 1;
    }
    Ok(done)
}

/// The user selects a record. It becomes a manual link, and no later automatic
/// matching will touch it.
#[tauri::command]
pub async fn review_confirm(
    state: State<'_, AppState>,
    store_entry_id: String,
    igdb_id: i64,
) -> Result<(), AppError> {
    confirm_one(&state, &store_entry_id, igdb_id).await
}

/// The body that "confirm one" and "confirm many" share. Because it is the same
/// body, the batch takes no short cut against the one-at-a-time path: it creates
/// the same record and writes the same `manual` link.
async fn confirm_one(state: &AppState, store_entry_id: &str, igdb_id: i64) -> Result<(), AppError> {
    let entry_id = parse_entry_id(store_entry_id)?;
    let entry = StoreEntryRepository(&state.db)
        .find(entry_id)
        .await?
        .ok_or_else(|| AppError::Message("that entry no longer exists".to_owned()))?;

    let games = GameRepository(&state.db);
    let game_id = match games.find_by_igdb(igdb_id).await? {
        Some(existing) => existing.id,
        None => {
            let (credentials, token) = igdb_session(state).await?;
            let meta = state.igdb.game(&credentials, &token, igdb_id).await?;
            let game = match meta {
                Some(meta) => domain::Game {
                    id: domain::GameId::new(),
                    canonical_title: meta.name.clone(),
                    sort_title: domain::matching::normalize(&meta.name),
                    igdb_id: Some(meta.igdb_id),
                    cover_url: meta.cover_url,
                    summary: meta.summary,
                    released_at: meta.released_at,
                    genres: meta.genres,
                },
                None => identity::local_game(&entry),
            };
            games.upsert(&game).await?;
            game.id
        }
    };

    link_manually(state, entry_id, game_id).await
}

/// "This game is not in IGDB": the code makes a record for it with the title of
/// the store, so that it goes out of the queue and can have a status as any
/// other game.
#[tauri::command]
pub async fn review_without_metadata(
    state: State<'_, AppState>,
    store_entry_id: String,
) -> Result<(), AppError> {
    let entry_id = parse_entry_id(&store_entry_id)?;
    let entry = StoreEntryRepository(&state.db)
        .find(entry_id)
        .await?
        .ok_or_else(|| AppError::Message("that entry no longer exists".to_owned()))?;

    let game = identity::local_game(&entry);
    GameRepository(&state.db).upsert(&game).await?;
    link_manually(&state, entry_id, game.id).await
}

async fn link_manually(
    state: &AppState,
    entry_id: StoreEntryId,
    game_id: domain::GameId,
) -> Result<(), AppError> {
    GameLinkRepository(&state.db)
        .set_manual(&GameLink {
            game_id,
            store_entry_id: entry_id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await?;
    MatchCandidateRepository(&state.db).clear(entry_id).await?;
    Ok(())
}

fn parse_entry_id(raw: &str) -> Result<StoreEntryId, AppError> {
    Uuid::parse_str(raw)
        .map(StoreEntryId::from_uuid)
        .map_err(|_| AppError::Message("invalid entry identifier".to_owned()))
}
