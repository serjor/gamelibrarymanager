//! Disconnecting a store removes access to it without removing the library data.
//!
//! The account is no longer active, its credential is gone, and the next
//! synchronisation does not call its connector. The game, its link and the
//! state written by the user remain.

use std::collections::HashMap;
use std::sync::Arc;

use connectors::SteamConnector;
use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, PlayStatus, StoreAccount, StoreAccountId,
    StoreConnector, StoreEntry, StoreEntryId, StoreId, UserState,
};
use gamelibrarymanager_lib::testing::{
    Silent, credential_key, disconnect_account_for, sync_stores,
};
use secrets::{EncryptedFileStore, SecretStore};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, StoreAccountRepository,
    StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;
use wiremock::MockServer;

async fn account(db: &Database) -> StoreAccount {
    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: "76561197960287930".to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(db)
        .upsert(&account)
        .await
        .expect("add the account");
    StoreAccount { id, ..account }
}

#[tokio::test]
async fn disconnecting_removes_access_and_keeps_the_library() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = Database::in_memory().await.expect("database");
    let secrets = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "a long passphrase")
        .expect("open the store");
    let account = account(&db).await;

    let game = Game {
        id: GameId::new(),
        canonical_title: "Disco Elysium".to_owned(),
        sort_title: "disco elysium".to_owned(),
        igdb_id: Some(115_653),
        cover_url: None,
        summary: None,
        released_at: None,
        genres: vec!["RPG".to_owned()],
    };
    GameRepository(&db)
        .upsert(&game)
        .await
        .expect("add the record");

    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id: account.id,
        store: StoreId::Steam,
        store_app_id: "632470".to_owned(),
        kind: EntryKind::Owned,
        title: game.canonical_title.clone(),
        playtime_minutes: Some(1200),
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({ "appid": 632470 }),
    };
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&entry))
        .await
        .expect("add the copy");
    GameLinkRepository(&db)
        .set_manual(&GameLink {
            game_id: game.id,
            store_entry_id: entry.id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await
        .expect("link the copy");

    let before = UserState {
        game_id: game.id,
        status: Some(PlayStatus::Playing),
        rating: Some(9),
        notes: Some("at chapter 3".to_owned()),
        started_at: Some(OffsetDateTime::now_utc()),
        finished_at: None,
    };
    UserStateRepository(&db)
        .save(&before)
        .await
        .expect("keep the user state");
    secrets
        .set(&credential_key(&account), "the credential")
        .expect("keep the credential");

    disconnect_account_for(&db, &secrets, StoreId::Steam, &account.account_ref)
        .await
        .expect("disconnect the account");

    assert!(
        StoreAccountRepository(&db)
            .active()
            .await
            .expect("list active accounts")
            .is_empty()
    );
    assert!(
        secrets
            .get(&credential_key(&account))
            .expect("read the credential")
            .is_none(),
        "disconnecting removes the credential"
    );
    assert!(
        StoreEntryRepository(&db)
            .active(EntryKind::Owned)
            .await
            .expect("list active copies")
            .is_empty(),
        "the copies are logically deleted"
    );
    assert_eq!(
        GameLinkRepository(&db)
            .for_game(game.id)
            .await
            .expect("read the link")
            .len(),
        1,
        "disconnecting does not remove the link"
    );
    assert_eq!(
        UserStateRepository(&db)
            .find(game.id)
            .await
            .expect("read the user state")
            .expect("the state remains"),
        before
    );

    let row = LibraryRepository(&db)
        .all()
        .await
        .expect("read the library")
        .pop()
        .expect("the record remains");
    assert!(row.owned_stores.is_empty());
    assert_eq!(row.notes.as_deref(), Some("at chapter 3"));

    let server = MockServer::start().await;
    let connector =
        SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri());
    let mut connectors = HashMap::new();
    connectors.insert(
        StoreId::Steam,
        Arc::new(connector) as Arc<dyn StoreConnector>,
    );
    let report = sync_stores(&db, &secrets, &connectors, &Silent)
        .await
        .expect("synchronisation after disconnect");
    assert!(report.failures.is_empty());
    assert_eq!(
        server.received_requests().await.unwrap_or_default().len(),
        0,
        "a disconnected store is not asked on the next synchronisation"
    );
}
