//! Records with no IGDB, and what occurs when IGDB comes later.
//!
//! To block all of the application until the user has Twitch credentials agrees
//! with the design — the record comes from the matching — but it is very hard at
//! the first start. The other option has one concrete risk and it is the risk
//! that this file tests: `user_state` is attached to the `game_id`, thus if the
//! metadata of a local record made a new record, the user would lose what they
//! had written on it.
use domain::{EntryKind, GameId};
use domain::{
    PlayStatus, StoreAccount, StoreAccountId, StoreEntry, StoreEntryId, StoreId, UserState,
};
use gamelibrarymanager_lib::testing::{Silent, resolve, resolve_local};
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use storage::Database;
use storage::repositories::{
    GameRepository, LibraryRepository, StoreAccountRepository, StoreEntryRepository,
    UserStateRepository,
};
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const IGDB_EXTERNAL: &str = include_str!("fixtures/igdb_external_witcher3.json");
const IGDB_SEARCH: &str = include_str!("fixtures/igdb_search_witcher3.json");
const IGDB_GAME: &str = include_str!("fixtures/igdb_game_witcher3.json");

async fn base() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("open the database");
    (dir, db)
}

/// Adds an account and attaches to it a copy with the title given.
async fn copia(db: &Database, store: StoreId, app_id: &str, title: &str) -> StoreEntryId {
    let account = StoreAccount {
        id: StoreAccountId::new(),
        store,
        account_ref: format!("account-{}", store.as_str()),
        display_name: None,
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let account_id = StoreAccountRepository(db)
        .upsert(&account)
        .await
        .expect("add the account");

    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind: EntryKind::Owned,
        title: title.to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::Value::Null,
    };
    StoreEntryRepository(db)
        .upsert_many(std::slice::from_ref(&entry))
        .await
        .expect("volcar copia");
    entry.id
}

async fn servidor_igdb() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains("292030"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(IGDB_EXTERNAL, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("where id = 1942"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(IGDB_GAME, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("search \"The Witcher 3"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(IGDB_SEARCH, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
    server
}

fn cliente(server: &MockServer) -> IgdbClient {
    IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()))
}

fn credenciales() -> IgdbCredentials {
    IgdbCredentials {
        client_id: "CLIENTE".to_owned(),
        client_secret: "SECRETO".to_owned(),
    }
}

fn token() -> IgdbToken {
    IgdbToken {
        access_token: "TOKEN".to_owned(),
        expires_at: OffsetDateTime::now_utc().unix_timestamp() + 3600,
    }
}

#[tokio::test]
async fn with_no_igdb_the_library_is_visible_and_deduplicates_by_title() {
    let (_dir, db) = base().await;
    copia(&db, StoreId::Steam, "292030", "The Witcher 3: Wild Hunt").await;
    copia(
        &db,
        StoreId::Gog,
        "1495134320",
        "The Witcher 3: Wild Hunt - Complete Edition",
    )
    .await;
    copia(&db, StoreId::Steam, "105600", "Terraria").await;

    let report = resolve_local(&db, &Silent)
        .await
        .expect("match with no IGDB");
    assert_eq!(report.linked, 3);

    let library = LibraryRepository(&db).all().await.expect("library");
    assert_eq!(library.len(), 2, "The Witcher 3 and Terraria");

    let witcher = library
        .iter()
        .find(|row| row.sort_title.contains("witcher"))
        .expect("the record of The Witcher 3");
    assert_eq!(
        witcher.owned_stores,
        vec!["gog".to_owned(), "steam".to_owned()],
        "with no IGDB, the normalisation of titles already joins the two stores"
    );
    assert_eq!(
        witcher.cover_url, None,
        "a local record does not invent metadata that it does not have"
    );
}

#[tokio::test]
async fn when_igdb_is_configured_the_record_gets_metadata_and_keeps_the_status() {
    let (_dir, db) = base().await;
    copia(&db, StoreId::Steam, "292030", "The Witcher 3: Wild Hunt").await;
    copia(
        &db,
        StoreId::Gog,
        "1495134320",
        "The Witcher 3: Wild Hunt - Complete Edition",
    )
    .await;

    // --- the first start, with no IGDB: the user can already mark a status ---
    resolve_local(&db, &Silent)
        .await
        .expect("match with no IGDB");
    let library = LibraryRepository(&db).all().await.expect("library");
    let local_record: GameId = library[0].game_id;

    UserStateRepository(&db)
        .save(&UserState {
            game_id: local_record,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("at the second act".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("keep the status");

    // --- and later they configure IGDB ---
    let server = servidor_igdb().await;
    resolve(&db, &cliente(&server), &credenciales(), &token(), &Silent)
        .await
        .expect("match with IGDB");

    let library = LibraryRepository(&db).all().await.expect("library");
    assert_eq!(library.len(), 1, "there is still one record");

    let row = &library[0];
    assert_eq!(
        row.game_id, local_record,
        "the record gets its metadata in place: if a second record were made, the \
         status of the user would stay attached to a record that nobody sees"
    );
    assert_eq!(row.title, "The Witcher 3: Wild Hunt");
    assert!(
        row.cover_url.is_some(),
        "now it does have a cover, which is what IGDB gives"
    );
    assert_eq!(row.status, Some(PlayStatus::Playing));
    assert_eq!(row.rating, Some(9));
    assert_eq!(row.notes.as_deref(), Some("at the second act"));
    assert_eq!(
        row.owned_stores,
        vec!["gog".to_owned(), "steam".to_owned()],
        "and the two stores are still attached to it"
    );

    assert_eq!(
        GameRepository(&db).all().await.expect("records").len(),
        1,
        "no orphan local record can stay in the database"
    );
}

#[tokio::test]
async fn what_igdb_does_not_recognise_does_not_go_out_of_the_library() {
    let (_dir, db) = base().await;
    // Terraria has no join in the IGDB fixtures: the search comes back empty.
    copia(&db, StoreId::Steam, "105600", "Terraria").await;

    resolve_local(&db, &Silent)
        .await
        .expect("match with no IGDB");
    assert_eq!(
        LibraryRepository(&db).all().await.expect("library").len(),
        1
    );

    let server = servidor_igdb().await;
    resolve(&db, &cliente(&server), &credenciales(), &token(), &Silent)
        .await
        .expect("match with IGDB");

    let library = LibraryRepository(&db).all().await.expect("library");
    assert_eq!(
        library.len(),
        1,
        "that IGDB does not know it is not a reason to take from the user a game \
         that they were already seeing"
    );
    assert_eq!(library[0].title, "Terraria");
    assert_eq!(library[0].owned_stores, vec!["steam".to_owned()]);
}
