//! The "done when" of phase 6: **a game owned in Steam and in GOG has one
//! record, with the two store badges**.
//!
//! It is the first real test of the deduplication between stores, which is the
//! core of the product, and thus it is tested from end to end: two accounts, two
//! connectors, a synchronisation, a match and the query that shows the library.
//! No test touches the network: all of the answers are recorded.
//!
//! The pair selected is not easy by accident. Steam sells "The Witcher 3: Wild
//! Hunt" and GOG sells "The Witcher 3: Wild Hunt - Complete Edition": the titles
//! do not agree, and that they end in the same record depends on the
//! normalisation holding "Complete Edition" as packaging and not as a different
//! game.
use connectors::{GogConnector, SteamConnector};
use domain::{StoreAccount, StoreAccountId, StoreId};
use gamelibrarymanager_lib::testing::{Silent, SyncReport, credential_key, resolve, sync_account};
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use secrets::{EncryptedFileStore, SecretStore};
use storage::Database;
use storage::repositories::{LibraryRepository, StoreAccountRepository};
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const STEAM_OWNED: &str =
    include_str!("../../crates/connectors/tests/fixtures/steam_owned_games.json");
const STEAM_WISHLIST: &str =
    include_str!("../../crates/connectors/tests/fixtures/steam_wishlist.json");
const STEAM_DETAILS: &str =
    include_str!("../../crates/connectors/tests/fixtures/steam_app_details.json");

const GOG_RELEASES: &str = include_str!("../../crates/connectors/tests/fixtures/gog_releases.json");
const GOG_RELEASES_2: &str =
    include_str!("../../crates/connectors/tests/fixtures/gog_releases_page2.json");
const GOG_PRODUCTS: &str = include_str!("../../crates/connectors/tests/fixtures/gog_products.json");

const IGDB_EXTERNAL: &str = include_str!("fixtures/igdb_external_witcher3.json");
const IGDB_SEARCH: &str = include_str!("fixtures/igdb_search_witcher3.json");
const IGDB_GAME: &str = include_str!("fixtures/igdb_game_witcher3.json");

const STEAM_ID: &str = "76561197960287930";
const GOG_USER_ID: &str = "51000000000000000";
/// The Steam appid of The Witcher 3, which is the appid that IGDB can join.
const APPID_WITCHER3: &str = "292030";

async fn responds(server: &MockServer, verbo: &str, route: &str, body: &'static str) {
    Mock::given(method(verbo))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(server)
        .await;
}

async fn steam_server_mock() -> MockServer {
    let server = MockServer::start().await;
    for (route, body) in [
        ("/IPlayerService/GetOwnedGames/v1/", STEAM_OWNED),
        ("/IWishlistService/GetWishlist/v1/", STEAM_WISHLIST),
        ("/api/appdetails", STEAM_DETAILS),
    ] {
        responds(&server, "GET", route, body).await;
    }
    server
}

async fn gog_server_mock() -> MockServer {
    let server = MockServer::start().await;
    let releases = format!("/users/{GOG_USER_ID}/releases");
    Mock::given(method("GET"))
        .and(path(releases.clone()))
        .and(wiremock::matchers::query_param("page_token", "PAGE_2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GOG_RELEASES_2, "application/json"))
        .mount(&server)
        .await;
    responds(&server, "GET", &releases, GOG_RELEASES).await;
    responds(&server, "GET", "/products", GOG_PRODUCTS).await;
    server
}

/// IGDB with the two methods that the matching uses: the external identifier for
/// Steam and the search by name for GOG, which has no join with IGDB.
///
/// Every game that is not The Witcher 3 answers empty deliberately: thus the
/// record that appears at the end can come only from the deduplication that this
/// test examines.
async fn igdb_mock() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains(APPID_WITCHER3))
        .respond_with(ResponseTemplate::new(200).set_body_raw(IGDB_EXTERNAL, "application/json"))
        .mount(&server)
        .await;
    responds(&server, "POST", "/external_games", "[]").await;

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
    responds(&server, "POST", "/games", "[]").await;

    server
}

fn account(store: StoreId, account_ref: &str) -> StoreAccount {
    StoreAccount {
        id: StoreAccountId::new(),
        store,
        account_ref: account_ref.to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    }
}

#[tokio::test]
async fn a_game_in_steam_and_in_gog_is_one_record_with_two_badges() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("open the database");
    let secrets = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "a long passphrase")
        .expect("open the store");

    // --- two accounts, one for each store ---
    let repo = StoreAccountRepository(&db);
    let mut steam = account(StoreId::Steam, STEAM_ID);
    steam.id = repo.upsert(&steam).await.expect("alta de Steam");
    let mut gog = account(StoreId::Gog, GOG_USER_ID);
    gog.id = repo.upsert(&gog).await.expect("alta de GOG");

    secrets
        .set(&credential_key(&steam), r#"{"api_key":"CLAVE"}"#)
        .expect("the Steam credential");
    secrets
        .set(&credential_key(&gog), &credencial_gog())
        .expect("the GOG credential");

    // --- synchronise the two ---
    let steam_server = steam_server_mock().await;
    let gog_server = gog_server_mock().await;

    let conector_steam = SteamConnector::new(reqwest::Client::new())
        .with_bases(steam_server.uri(), steam_server.uri());
    let conector_gog = GogConnector::new(reqwest::Client::new()).with_bases(&gog_server.uri());

    let mut report = SyncReport::default();
    sync_account(&db, &secrets, &conector_steam, &steam, &mut report)
        .await
        .expect("sincronizar Steam");
    sync_account(&db, &secrets, &conector_gog, &gog, &mut report)
        .await
        .expect("sincronizar GOG");

    assert_eq!(report.owned, 5, "3 Steam copies and 2 GOG copies");

    // --- emparejar contra IGDB ---
    let igdb_server = igdb_mock().await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(igdb_server.uri(), format!("{}/token", igdb_server.uri()));

    resolve(&db, &igdb, &credenciales_igdb(), &token_igdb(), &Silent)
        .await
        .expect("match");

    // --- and what must be visible: ONE record with TWO badges ---
    let biblioteca = LibraryRepository(&db).all().await.expect("library");

    assert_eq!(
        biblioteca.len(),
        1,
        "only The Witcher 3 has a record; the others stay unmatched deliberately"
    );

    let witcher = &biblioteca[0];
    assert_eq!(witcher.title, "The Witcher 3: Wild Hunt");
    assert_eq!(
        witcher.owned_stores,
        vec!["gog".to_owned(), "steam".to_owned()],
        "the Steam copy and the GOG copy are attached to the same record"
    );
}

#[tokio::test]
async fn a_second_match_does_not_split_the_record() {
    // The deduplication must be idempotent: if a second record appeared at the
    // second match, the user would see their game two times and would lose the
    // status attached to the first record.
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("open the database");
    let secrets = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "a long passphrase")
        .expect("open the store");

    let repo = StoreAccountRepository(&db);
    let mut steam = account(StoreId::Steam, STEAM_ID);
    steam.id = repo.upsert(&steam).await.expect("alta de Steam");
    let mut gog = account(StoreId::Gog, GOG_USER_ID);
    gog.id = repo.upsert(&gog).await.expect("alta de GOG");
    secrets
        .set(&credential_key(&steam), r#"{"api_key":"CLAVE"}"#)
        .expect("the Steam credential");
    secrets
        .set(&credential_key(&gog), &credencial_gog())
        .expect("the GOG credential");

    let steam_server = steam_server_mock().await;
    let gog_server = gog_server_mock().await;
    let igdb_server = igdb_mock().await;

    let conector_steam = SteamConnector::new(reqwest::Client::new())
        .with_bases(steam_server.uri(), steam_server.uri());
    let conector_gog = GogConnector::new(reqwest::Client::new()).with_bases(&gog_server.uri());
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(igdb_server.uri(), format!("{}/token", igdb_server.uri()));

    for _ in 0..2 {
        let mut report = SyncReport::default();
        sync_account(&db, &secrets, &conector_steam, &steam, &mut report)
            .await
            .expect("sincronizar Steam");
        sync_account(&db, &secrets, &conector_gog, &gog, &mut report)
            .await
            .expect("sincronizar GOG");
        resolve(&db, &igdb, &credenciales_igdb(), &token_igdb(), &Silent)
            .await
            .expect("match");
    }

    let biblioteca = LibraryRepository(&db).all().await.expect("library");
    assert_eq!(biblioteca.len(), 1, "two passes still give one record");
    assert_eq!(
        biblioteca[0].owned_stores,
        vec!["gog".to_owned(), "steam".to_owned()]
    );
}

fn credencial_gog() -> String {
    let futuro = OffsetDateTime::now_utc().unix_timestamp() + 3600;
    format!(
        r#"{{"client_id":"46899977096215655","client_secret":"SECRETO_DEL_USUARIO",
             "access_token":"ACCESO","refresh_token":"REFRESCO",
             "user_id":"{GOG_USER_ID}","expires_at":{futuro}}}"#
    )
}

fn credenciales_igdb() -> IgdbCredentials {
    IgdbCredentials {
        client_id: "CLIENTE".to_owned(),
        client_secret: "SECRETO".to_owned(),
    }
}

fn token_igdb() -> IgdbToken {
    IgdbToken {
        access_token: "TOKEN".to_owned(),
        expires_at: OffsetDateTime::now_utc().unix_timestamp() + 3600,
    }
}
