//! The two disasters that the division into four layers must prevent: that a
//! new match deletes what the user wrote, and that the automatic matching
//! overwrites a manual correction.

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, PlayStatus, StoreAccount, StoreAccountId,
    StoreEntry, StoreEntryId, StoreId, UserState,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, StoreAccountRepository, StoreEntryRepository,
    UserStateRepository,
};
use time::OffsetDateTime;

async fn account(db: &Database, store: StoreId) -> StoreAccountId {
    StoreAccountRepository(db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store,
            account_ref: format!("account-{}", store.as_str()),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("add the account")
}

fn entry(account_id: StoreAccountId, store: StoreId, app_id: &str) -> StoreEntry {
    StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind: EntryKind::Owned,
        title: "Disco Elysium".to_owned(),
        playtime_minutes: Some(1200),
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({ "appid": app_id }),
    }
}

#[tokio::test]
async fn the_migrations_apply_and_revert() {
    let db = Database::in_memory().await.expect("open the database");

    // Applied by `open`: the tables answer.
    assert_eq!(GameRepository(&db).all().await.expect("query").len(), 0);

    db.undo_all().await.expect("revert the migrations");

    // Reverted: the table no longer exists and the query fails.
    assert!(
        GameRepository(&db).all().await.is_err(),
        "after the revert there must be no schema"
    );
}

#[tokio::test]
async fn a_new_match_does_not_touch_the_user_status() {
    let db = Database::in_memory().await.expect("open the database");
    let steam = account(&db, StoreId::Steam).await;
    let gog = account(&db, StoreId::Gog).await;

    // The same game, bought in two stores.
    let in_steam = entry(steam, StoreId::Steam, "632470");
    let in_gog = entry(gog, StoreId::Gog, "1512395083");
    StoreEntryRepository(&db)
        .upsert_many(&[in_steam.clone(), in_gog.clone()])
        .await
        .expect("write the entries");

    // One record for the two copies.
    let game_id = GameId::new();
    GameRepository(&db)
        .upsert(&Game {
            id: game_id,
            canonical_title: "Disco Elysium".to_owned(),
            sort_title: "disco elysium".to_owned(),
            igdb_id: Some(115_653),
            cover_url: None,
            summary: None,
            released_at: None,
            genres: Vec::new(),
        })
        .await
        .expect("add the record");

    let links = vec![
        GameLink {
            game_id,
            store_entry_id: in_steam.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        },
        GameLink {
            game_id,
            store_entry_id: in_gog.id,
            confidence: 0.9,
            method: LinkMethod::Auto,
        },
    ];
    let repo = GameLinkRepository(&db);
    repo.rebuild_auto(&links).await.expect("match");
    assert_eq!(
        repo.for_game(game_id).await.expect("read the links").len(),
        2
    );
    assert_eq!(repo.unlinked_entry_count().await.expect("count"), 0);

    // The user writes their status.
    UserStateRepository(&db)
        .save(&UserState {
            game_id,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("at chapter 3".to_owned()),
            started_at: Some(OffsetDateTime::now_utc()),
            finished_at: None,
        })
        .await
        .expect("keep the status");

    // All of the matching is made again, two times.
    repo.rebuild_auto(&links).await.expect("match again");
    repo.rebuild_auto(&links).await.expect("match a third time");

    let state = UserStateRepository(&db)
        .find(game_id)
        .await
        .expect("read the status")
        .expect("the user status must survive the new match");
    assert_eq!(state.status, Some(PlayStatus::Playing));
    assert_eq!(state.rating, Some(9));
    assert_eq!(state.notes.as_deref(), Some("at chapter 3"));

    // And the initial store data stays unchanged.
    let stored = StoreEntryRepository(&db)
        .find(in_steam.id)
        .await
        .expect("read the entry")
        .expect("the store entry is not touched by a new match");
    assert_eq!(stored.raw, in_steam.raw);
}

#[tokio::test]
async fn the_automatic_matching_does_not_overwrite_a_manual_correction() {
    let db = Database::in_memory().await.expect("open the database");
    let steam = account(&db, StoreId::Steam).await;
    let in_steam = entry(steam, StoreId::Steam, "632470");
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&in_steam))
        .await
        .expect("write the entry");

    let wrong = GameId::new();
    let right = GameId::new();
    for (id, title) in [
        (wrong, "Disco Elysium"),
        (right, "Disco Elysium: The Final Cut"),
    ] {
        GameRepository(&db)
            .upsert(&Game {
                id,
                canonical_title: title.to_owned(),
                sort_title: title.to_lowercase(),
                igdb_id: None,
                cover_url: None,
                summary: None,
                released_at: None,
                genres: Vec::new(),
            })
            .await
            .expect("add the record");
    }

    let repo = GameLinkRepository(&db);
    // The user corrects it by hand.
    repo.set_manual(&GameLink {
        game_id: right,
        store_entry_id: in_steam.id,
        confidence: 1.0,
        method: LinkMethod::Manual,
    })
    .await
    .expect("a manual correction");

    // The automatic matching proposes its link again, and it must not win.
    repo.rebuild_auto(&[GameLink {
        game_id: wrong,
        store_entry_id: in_steam.id,
        confidence: 0.95,
        method: LinkMethod::Auto,
    }])
    .await
    .expect("match again");

    let links = repo.all().await.expect("read the links");
    assert_eq!(links.len(), 1, "one entry can have only one link");
    assert_eq!(links[0].game_id, right, "the manual correction controls");
    assert_eq!(links[0].method, LinkMethod::Manual);
}

#[tokio::test]
async fn a_second_synchronisation_does_not_duplicate() {
    let db = Database::in_memory().await.expect("open the database");
    let steam = account(&db, StoreId::Steam).await;
    let first = entry(steam, StoreId::Steam, "632470");

    let repo = StoreEntryRepository(&db);
    repo.upsert_many(std::slice::from_ref(&first))
        .await
        .expect("the first");

    // The second synchronisation: the same game, a new id and more hours.
    let mut second = entry(steam, StoreId::Steam, "632470");
    second.playtime_minutes = Some(1500);
    repo.upsert_many(&[second]).await.expect("the second");

    let active_entries = repo.active(EntryKind::Owned).await.expect("list");
    assert_eq!(
        active_entries.len(),
        1,
        "there cannot be two rows of the same game"
    );
    assert_eq!(active_entries[0].id, first.id, "the initial row is kept");
    assert_eq!(active_entries[0].playtime_minutes, Some(1500));
}

#[tokio::test]
async fn what_goes_out_of_the_store_is_marked_and_not_deleted() {
    let db = Database::in_memory().await.expect("open the database");
    let steam = account(&db, StoreId::Steam).await;
    let removed = entry(steam, StoreId::Steam, "632470");
    let stays = entry(steam, StoreId::Steam, "292030");

    let repo = StoreEntryRepository(&db);
    repo.upsert_many(&[removed.clone(), stays.clone()])
        .await
        .expect("write");

    let deleted = repo
        .soft_delete_missing(steam, EntryKind::Owned, &["292030".to_owned()])
        .await
        .expect("a logical delete");

    assert_eq!(deleted, 1);
    assert_eq!(repo.active(EntryKind::Owned).await.expect("list").len(), 1);
    assert!(
        repo.find(removed.id).await.expect("find").is_some(),
        "the row continues to exist, it is only marked"
    );
}
