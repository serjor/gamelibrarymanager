//! A save writes one row and gives that row back.
//!
//! Before, `set_user_state` gave back nothing and the interface answered with a
//! complete refresh: all of the library, all of the review queue and all of the
//! prices, eight commands, to see one word change on one row. The answer is now
//! the row itself, and the batch writes all of the selection in one call.

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, PlayStatus, StoreAccount, StoreAccountId,
    StoreEntry, StoreEntryId, StoreId, UserState,
};
use gamelibrarymanager_lib::testing::{StateUpdate, save_states};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, StoreAccountRepository,
    StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;

/// A library of `count` games, each one with a copy in Steam. The copy is what
/// gives the row its badges and its hours, which is exactly what a save must
/// not lose when it gives the row back.
async fn library_of(db: &Database, count: usize) -> Vec<GameId> {
    let account = StoreAccountRepository(db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store: StoreId::Steam,
            account_ref: "account".to_owned(),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("account");

    let mut ids = Vec::with_capacity(count);
    let mut entries = Vec::with_capacity(count);
    let mut links = Vec::with_capacity(count);

    for i in 0..count {
        let record = Game {
            id: GameId::new(),
            canonical_title: format!("Game {i:04}"),
            sort_title: format!("game {i:04}"),
            igdb_id: None,
            cover_url: None,
            summary: None,
            released_at: None,
            genres: vec!["Shooter".to_owned()],
        };
        GameRepository(db).upsert(&record).await.expect("record");

        let entry = StoreEntry {
            id: StoreEntryId::new(),
            account_id: account,
            store: StoreId::Steam,
            store_app_id: i.to_string(),
            kind: EntryKind::Owned,
            title: record.canonical_title.clone(),
            playtime_minutes: Some(120),
            acquired_at: None,
            cover_url: None,
            store_url: None,
            raw: serde_json::json!({}),
        };
        links.push(GameLink {
            game_id: record.id,
            store_entry_id: entry.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        });
        entries.push(entry);
        ids.push(record.id);
    }

    StoreEntryRepository(db)
        .upsert_many(&entries)
        .await
        .expect("copies");
    GameLinkRepository(db)
        .rebuild_auto(&links)
        .await
        .expect("links");
    ids
}

fn update(game_id: GameId, status: PlayStatus) -> StateUpdate {
    StateUpdate {
        game_id: game_id.as_uuid().to_string(),
        status: Some(status),
        rating: None,
        notes: None,
    }
}

#[tokio::test]
async fn the_row_that_a_save_gives_back_is_the_row_of_the_list() {
    let db = Database::in_memory().await.expect("database");
    let ids = library_of(&db, 3).await;

    let saved = save_states(
        &db,
        &[StateUpdate {
            game_id: ids[1].as_uuid().to_string(),
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("half done".to_owned()),
        }],
    )
    .await
    .expect("the save");

    assert_eq!(saved.len(), 1);
    let row = &saved[0];
    assert_eq!(row.status, Some(PlayStatus::Playing));
    assert_eq!(row.rating, Some(9));
    assert_eq!(row.notes.as_deref(), Some("half done"));
    // The save gives the complete row and not the four fields that it wrote:
    // the store badges and the hours come with it, thus the interface has
    // nothing left to ask for.
    assert_eq!(row.owned_stores, vec!["steam".to_owned()]);
    assert_eq!(row.playtime_minutes, 120);

    // And it is the same row, field by field, as the one that the list gives.
    // The two go through one query, thus they cannot become different.
    let listed = LibraryRepository(&db)
        .all()
        .await
        .expect("the library")
        .into_iter()
        .find(|listed| listed.game_id == ids[1])
        .expect("the game is in the library");
    assert_eq!(&listed, row);
}

#[tokio::test]
async fn a_batch_writes_all_of_the_selection_and_gives_back_only_those_rows() {
    let db = Database::in_memory().await.expect("database");
    let ids = library_of(&db, 30).await;

    let updates: Vec<StateUpdate> = ids
        .iter()
        .map(|id| update(*id, PlayStatus::Abandoned))
        .collect();
    let saved = save_states(&db, &updates).await.expect("the batch");

    assert_eq!(saved.len(), 30);
    assert!(
        saved
            .iter()
            .all(|row| row.status == Some(PlayStatus::Abandoned))
    );

    // Nothing outside the selection is touched: a second batch over three games
    // gives back three rows, and the other twenty-seven keep what they had.
    let three: Vec<StateUpdate> = ids[..3]
        .iter()
        .map(|id| update(*id, PlayStatus::Finished))
        .collect();
    let saved = save_states(&db, &three).await.expect("the second batch");
    assert_eq!(saved.len(), 3);

    let finished = LibraryRepository(&db)
        .all()
        .await
        .expect("the library")
        .into_iter()
        .filter(|row| row.status == Some(PlayStatus::Finished))
        .count();
    assert_eq!(finished, 3);
}

#[tokio::test]
async fn a_batch_that_fails_writes_nothing() {
    let db = Database::in_memory().await.expect("database");
    let ids = library_of(&db, 3).await;

    let mut updates: Vec<StateUpdate> = ids
        .iter()
        .map(|id| update(*id, PlayStatus::Finished))
        .collect();
    updates.push(StateUpdate {
        game_id: "not a uuid".to_owned(),
        status: Some(PlayStatus::Finished),
        rating: None,
        notes: None,
    });

    save_states(&db, &updates)
        .await
        .expect_err("an identifier that is not a UUID must stop the batch");

    // The interface marked four games and none of them is written: half of a
    // selection written is worse than a message that says that nothing was.
    let marked = LibraryRepository(&db)
        .all()
        .await
        .expect("the library")
        .into_iter()
        .filter(|row| row.status.is_some())
        .count();
    assert_eq!(marked, 0);
}

#[tokio::test]
async fn a_save_keeps_the_dates_that_it_does_not_know() {
    let db = Database::in_memory().await.expect("database");
    let ids = library_of(&db, 1).await;
    let started = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("a date");

    UserStateRepository(&db)
        .save(&UserState {
            game_id: ids[0],
            status: Some(PlayStatus::Playing),
            rating: None,
            notes: None,
            started_at: Some(started),
            finished_at: None,
        })
        .await
        .expect("the first state");

    save_states(&db, &[update(ids[0], PlayStatus::Finished)])
        .await
        .expect("the save");

    // The form has no date field, thus the save writes the row again with the
    // dates that it read. Without that, a change of status would clear them.
    let state = UserStateRepository(&db)
        .find(ids[0])
        .await
        .expect("the state")
        .expect("the game has a state");
    assert_eq!(state.status, Some(PlayStatus::Finished));
    assert_eq!(state.started_at, Some(started));
}
