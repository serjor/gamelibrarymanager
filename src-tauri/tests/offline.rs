//! What the application does when the network does not help.
//!
//! The two "done when" of phase 5 that are not about the interface: nobody
//! overwrites what the user writes, and with no network the library is still
//! there. And a third one, from plan `0004`: a store that answers nothing is
//! also a store that does not help, and it must not hold the synchronisation.
use std::time::Duration;

use connectors::SteamConnector;
use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, PlayStatus, StoreAccount, StoreAccountId,
    StoreId, UserState,
};
use gamelibrarymanager_lib::testing::{SyncReport, credential_key, http_client_with, sync_account};
use secrets::{EncryptedFileStore, SecretStore};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, StoreAccountRepository,
    StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const OWNED: &str = include_str!("../../crates/connectors/tests/fixtures/steam_owned_games.json");
const WISHLIST: &str = include_str!("../../crates/connectors/tests/fixtures/steam_wishlist.json");
const DETAILS: &str = include_str!("../../crates/connectors/tests/fixtures/steam_app_details.json");

async fn escenario(dir: &std::path::Path) -> (Database, EncryptedFileStore, StoreAccount) {
    let db = Database::open(&dir.join("library.db"))
        .await
        .expect("open the database");

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: "76561197960287930".to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(&db)
        .upsert(&account)
        .await
        .expect("account");
    let account = StoreAccount { id, ..account };

    let store =
        EncryptedFileStore::open(&dir.join("secrets.bin"), "a long passphrase").expect("store");
    store
        .set(&credential_key(&account), r#"{"api_key":"CLAVE"}"#)
        .expect("credencial");

    (db, store, account)
}

async fn steam_server() -> MockServer {
    let server = MockServer::start().await;
    for (route, body) in [
        ("/IPlayerService/GetOwnedGames/v1/", OWNED),
        ("/IWishlistService/GetWishlist/v1/", WISHLIST),
        ("/api/appdetails", DETAILS),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .mount(&server)
            .await;
    }
    server
}

#[tokio::test]
async fn the_user_status_survives_a_complete_synchronisation() {
    let dir = tempfile::tempdir().expect("temporal");
    let (db, secrets, account) = escenario(dir.path()).await;
    let server = steam_server().await;
    let connector =
        SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri());

    sync_account(
        &db,
        &secrets,
        &connector,
        &account,
        &mut SyncReport::default(),
    )
    .await
    .expect("the first synchronisation");

    // One of the entries is matched by hand and the user marks it.
    let entry = StoreEntryRepository(&db)
        .active(EntryKind::Owned)
        .await
        .expect("entries")
        .into_iter()
        .next()
        .expect("there are entries");

    let record = Game {
        id: GameId::new(),
        canonical_title: "Disco Elysium".to_owned(),
        sort_title: "disco elysium".to_owned(),
        igdb_id: Some(115653),
        cover_url: None,
        summary: None,
        released_at: None,
        genres: vec!["RPG".to_owned()],
    };
    GameRepository(&db).upsert(&record).await.expect("record");
    GameLinkRepository(&db)
        .set_manual(&GameLink {
            game_id: record.id,
            store_entry_id: entry.id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await
        .expect("enlace");

    UserStateRepository(&db)
        .save(&UserState {
            game_id: record.id,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("at chapter 3".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("status");

    // And now it synchronises two more times, completely.
    for _ in 0..2 {
        sync_account(
            &db,
            &secrets,
            &connector,
            &account,
            &mut SyncReport::default(),
        )
        .await
        .expect("a new synchronisation");
    }

    let rows = LibraryRepository(&db).all().await.expect("library");
    let row = rows
        .iter()
        .find(|r| r.game_id == record.id)
        .expect("the record stays there");
    assert_eq!(row.status, Some(PlayStatus::Playing));
    assert_eq!(row.rating, Some(9));
    assert_eq!(row.notes.as_deref(), Some("at chapter 3"));
    assert_eq!(row.owned_stores, vec!["steam".to_owned()]);
}

#[tokio::test]
async fn with_no_network_the_library_is_visible_and_only_the_synchronisation_fails() {
    let dir = tempfile::tempdir().expect("temporal");
    let (db, secrets, account) = escenario(dir.path()).await;

    // The library is filled while the network is available.
    let server = steam_server().await;
    let connector =
        SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri());
    sync_account(
        &db,
        &secrets,
        &connector,
        &account,
        &mut SyncReport::default(),
    )
    .await
    .expect("the initial synchronisation");

    let entries = StoreEntryRepository(&db)
        .active(EntryKind::Owned)
        .await
        .expect("entries");
    let record = Game {
        id: GameId::new(),
        canonical_title: "Disco Elysium".to_owned(),
        sort_title: "disco elysium".to_owned(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: vec![],
    };
    GameRepository(&db).upsert(&record).await.expect("record");
    GameLinkRepository(&db)
        .rebuild_auto(&[GameLink {
            game_id: record.id,
            store_entry_id: entries[0].id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        }])
        .await
        .expect("enlace");

    // The network goes down: the server stops existing.
    drop(server);
    let down = SteamConnector::new(reqwest::Client::new())
        .with_bases("http://127.0.0.1:1", "http://127.0.0.1:1");

    let error = sync_account(&db, &secrets, &down, &account, &mut SyncReport::default())
        .await
        .expect_err("sin red, sincronizar falla");
    assert!(
        error.to_string().contains("could not contact"),
        "the error must say that it is the network: {error}"
    );

    // And the library still reads completely, which is what the user sees.
    let rows = LibraryRepository(&db)
        .all()
        .await
        .expect("the library with no network");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Disco Elysium");
}

/// A store that accepts the connection and then says nothing must not hold the
/// synchronisation for ever.
///
/// A store that is down gives an error immediately, and the test above proves
/// that. A store that answers nothing is worse: `reqwest` puts no limit of its
/// own, and the cancel flag does not reach inside a request.
///
/// The client of this test carries a short limit and not the thirty seconds of
/// the application, because a test that waits half a minute is a test that
/// nobody runs. It comes out of the same builder as the client of the
/// application, thus this test cannot pass while the application builds a
/// client with no limit.
///
/// It measures no time, as the conventions of `docs/testing/` say. It asserts
/// the error: with no limit the request waits for the answer that the server
/// holds, the synchronisation succeeds, and `expect_err` fails.
#[tokio::test]
async fn a_store_that_answers_nothing_does_not_hold_the_synchronisation() {
    let dir = tempfile::tempdir().expect("temporal");
    let (db, secrets, account) = escenario(dir.path()).await;

    // The library of the account is the first request of the synchronisation,
    // and it is the request that never arrives.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IPlayerService/GetOwnedGames/v1/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(OWNED, "application/json")
                .set_delay(Duration::from_secs(10)),
        )
        .mount(&server)
        .await;

    let connector = SteamConnector::new(http_client_with(Duration::from_millis(250)))
        .with_bases(server.uri(), server.uri());

    let error = sync_account(
        &db,
        &secrets,
        &connector,
        &account,
        &mut SyncReport::default(),
    )
    .await
    .expect_err("a store that answers nothing must give an error");

    assert!(
        error.to_string().contains("could not contact"),
        "the error must say that the store did not answer: {error}"
    );
}
