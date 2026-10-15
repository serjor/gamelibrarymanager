//! The stage of the Steam family, from end to end against a pretend Steam and a
//! real database.
//!
//! What these tests watch: that the family is not asked when it is switched
//! off, that a failure of the family is a result and never an error of the
//! synchronisation, that switching it on keeps nothing when the first read
//! fails, that a session of another account is refused, that disconnecting
//! Steam takes the family with it, that the export does not carry it, and that
//! the identity pass joins its games to IGDB so that a wish of GOG is marked.

use async_trait::async_trait;
use connectors::SteamConnector;
use connectors::steam::StoreToken;
use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, StoreAccount, StoreAccountId, StoreEntry,
    StoreEntryId, StoreId,
};
use gamelibrarymanager_lib::testing::{
    AppError, ExportFormat, FamilyOutcome, FamilyTokenSource, FixedToken, Silent, credential_key,
    disable_family, disconnect_account_for, enable_family, export_library_for, resolve,
    sync_family,
};
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use secrets::{EncryptedFileStore, SecretStore};
use storage::Database;
use storage::repositories::{
    ConnectorStateRepository, GameLinkRepository, GameRepository, LibraryRepository,
    SteamFamilyRepository, StoreAccountRepository, StoreEntryRepository,
};
use tempfile::TempDir;
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const GROUP: &str = include_str!("../../crates/connectors/tests/fixtures/steam_family_group.json");
const SHARED: &str =
    include_str!("../../crates/connectors/tests/fixtures/steam_family_shared_apps.json");
const NAMES: &str =
    include_str!("../../crates/connectors/tests/fixtures/steam_family_player_summaries.json");

/// The connected account of the fixtures.
const USER: &str = "76561197960287930";

struct Library {
    _dir: TempDir,
    db: Database,
    secrets: EncryptedFileStore,
}

async fn library() -> Library {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = Database::in_memory().await.expect("database");
    let secrets = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "a long passphrase")
        .expect("open the store");
    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: USER.to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(&db)
        .upsert(&account)
        .await
        .expect("add the account");
    let account = StoreAccount { id, ..account };
    secrets
        .set(&credential_key(&account), r#"{"api_key":"TEST_API_KEY"}"#)
        .expect("save the credential");
    Library {
        _dir: dir,
        db,
        secrets,
    }
}

async fn steam_server() -> MockServer {
    let server = MockServer::start().await;
    for (route, body) in [
        ("/IFamilyGroupsService/GetFamilyGroupForUser/v1/", GROUP),
        ("/IFamilyGroupsService/GetSharedLibraryApps/v1/", SHARED),
        ("/ISteamUser/GetPlayerSummaries/v2/", NAMES),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .mount(&server)
            .await;
    }
    server
}

fn steam(server: &MockServer) -> SteamConnector {
    SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri())
}

fn token_of(steam_id: &str) -> FixedToken {
    FixedToken(StoreToken {
        token: "TEST_STORE_TOKEN".to_owned(),
        steam_id: Some(steam_id.to_owned()),
        expires_at: None,
    })
}

/// The user closed the sign in.
struct Closed;

#[async_trait]
impl FamilyTokenSource for Closed {
    async fn token(&self) -> Result<Option<StoreToken>, AppError> {
        Ok(None)
    }
}

/// A source that must not be asked: the family is off.
struct Never;

#[async_trait]
impl FamilyTokenSource for Never {
    async fn token(&self) -> Result<Option<StoreToken>, AppError> {
        panic!("the family is switched off: nobody must ask for a token")
    }
}

async fn switched_on(lib: &Library, server: &MockServer) {
    let outcome = enable_family(
        &lib.db,
        &lib.secrets,
        &steam(server),
        &token_of(USER),
        &Silent,
    )
    .await
    .expect("switch the family on");
    assert_eq!(outcome, FamilyOutcome::Updated { games: 3 });
}

#[tokio::test]
async fn the_family_is_not_asked_when_it_is_switched_off() {
    let lib = library().await;
    let server = MockServer::start().await;

    let outcome = sync_family(&lib.db, &lib.secrets, &steam(&server), &Never, &Silent)
        .await
        .expect("the stage");

    assert_eq!(outcome, None);
    assert!(
        server
            .received_requests()
            .await
            .expect("requests")
            .is_empty()
    );
}

#[tokio::test]
async fn switching_on_reads_the_family_and_names_the_owners() {
    let lib = library().await;
    let server = steam_server().await;

    switched_on(&lib, &server).await;

    let apps = SteamFamilyRepository(&lib.db).apps().await.expect("list");
    let disco = apps
        .iter()
        .find(|app| app.app_id == "632470")
        .expect("Disco Elysium");
    let names: Vec<Option<&str>> = disco.owners.iter().map(|o| o.name.as_deref()).collect();
    assert_eq!(names, [Some("Ana"), Some("Luis")]);
    let settings = SteamFamilyRepository(&lib.db)
        .settings()
        .await
        .expect("settings")
        .expect("on");
    assert_eq!(
        settings.family_groupid.as_deref(),
        Some("1234567890123456789")
    );
    assert_eq!(settings.games, 3);
}

#[tokio::test]
async fn a_closed_sign_in_skips_the_family_and_keeps_the_last_list() {
    let lib = library().await;
    let server = steam_server().await;
    switched_on(&lib, &server).await;

    let outcome = sync_family(&lib.db, &lib.secrets, &steam(&server), &Closed, &Silent)
        .await
        .expect("a closed sign in is not an error of the synchronisation");

    let Some(FamilyOutcome::Skipped { reason }) = outcome else {
        panic!("skipped, and not {outcome:?}");
    };
    assert!(reason.contains("closed"), "{reason}");
    let settings = SteamFamilyRepository(&lib.db)
        .settings()
        .await
        .expect("settings")
        .expect("on");
    assert_eq!(settings.games, 3, "the list of the last stage stays");
    assert_eq!(settings.last_error.as_deref(), Some(reason.as_str()));
}

#[tokio::test]
async fn a_session_that_steam_refuses_is_a_result_and_not_an_error() {
    let lib = library().await;
    switched_on(&lib, &steam_server().await).await;

    let refusing = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IFamilyGroupsService/GetFamilyGroupForUser/v1/"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&refusing)
        .await;

    let outcome = sync_family(
        &lib.db,
        &lib.secrets,
        &steam(&refusing),
        &token_of(USER),
        &Silent,
    )
    .await
    .expect("a refusal of Steam is not an error of the synchronisation");

    let Some(FamilyOutcome::Skipped { reason }) = outcome else {
        panic!("skipped, and not {outcome:?}");
    };
    assert!(
        reason.contains("did not accept the store session"),
        "{reason}"
    );
}

#[tokio::test]
async fn a_session_of_another_account_is_refused_before_steam_is_asked() {
    let lib = library().await;
    switched_on(&lib, &steam_server().await).await;
    let server = MockServer::start().await;

    let outcome = sync_family(
        &lib.db,
        &lib.secrets,
        &steam(&server),
        &token_of("76561190000000009"),
        &Silent,
    )
    .await
    .expect("the stage");

    let Some(FamilyOutcome::Skipped { reason }) = outcome else {
        panic!("skipped, and not {outcome:?}");
    };
    assert!(reason.contains("another account"), "{reason}");
    assert!(
        server
            .received_requests()
            .await
            .expect("requests")
            .is_empty()
    );
}

#[tokio::test]
async fn with_the_steam_connector_off_the_family_is_skipped() {
    let lib = library().await;
    switched_on(&lib, &steam_server().await).await;
    ConnectorStateRepository(&lib.db)
        .set_enabled(StoreId::Steam, false)
        .await
        .expect("switch Steam off");

    let outcome = sync_family(
        &lib.db,
        &lib.secrets,
        &steam(&MockServer::start().await),
        &Never,
        &Silent,
    )
    .await
    .expect("the stage");

    assert!(matches!(outcome, Some(FamilyOutcome::Skipped { .. })));
}

#[tokio::test]
async fn switching_on_with_a_closed_sign_in_keeps_the_family_off() {
    let lib = library().await;

    let error = enable_family(
        &lib.db,
        &lib.secrets,
        &steam(&MockServer::start().await),
        &Closed,
        &Silent,
    )
    .await
    .expect_err("nothing to show");

    assert!(error.to_string().contains("closed"), "{error}");
    assert_eq!(
        SteamFamilyRepository(&lib.db)
            .settings()
            .await
            .expect("settings"),
        None
    );
}

#[tokio::test]
async fn switching_on_with_a_refused_session_keeps_the_family_off() {
    let lib = library().await;
    let refusing = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IFamilyGroupsService/GetFamilyGroupForUser/v1/"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&refusing)
        .await;

    enable_family(
        &lib.db,
        &lib.secrets,
        &steam(&refusing),
        &token_of(USER),
        &Silent,
    )
    .await
    .expect_err("Steam refused the session");

    assert_eq!(
        SteamFamilyRepository(&lib.db)
            .settings()
            .await
            .expect("settings"),
        None
    );
}

#[tokio::test]
async fn disconnecting_steam_switches_the_family_off() {
    let lib = library().await;
    switched_on(&lib, &steam_server().await).await;

    disconnect_account_for(&lib.db, &lib.secrets, StoreId::Steam, USER)
        .await
        .expect("disconnect");

    assert_eq!(
        SteamFamilyRepository(&lib.db)
            .settings()
            .await
            .expect("settings"),
        None
    );
}

#[tokio::test]
async fn switching_off_removes_the_list() {
    let lib = library().await;
    switched_on(&lib, &steam_server().await).await;

    disable_family(&lib.db).await.expect("switch off");

    assert!(
        SteamFamilyRepository(&lib.db)
            .apps()
            .await
            .expect("list")
            .is_empty()
    );
}

/// A record with one wished copy of GOG, and the IGDB record of the game.
/// Neither title is the title of Steam once normalised: only IGDB joins them.
async fn gog_wish(db: &Database, igdb_id: i64) -> GameId {
    let account = StoreAccountRepository(db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store: StoreId::Gog,
            account_ref: "gog-user".to_owned(),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("account");
    let game = Game {
        id: GameId::new(),
        canonical_title: "The Witcher III".to_owned(),
        sort_title: "the witcher iii".to_owned(),
        igdb_id: Some(igdb_id),
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    GameRepository(db).upsert(&game).await.expect("record");
    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id: account,
        store: StoreId::Gog,
        store_app_id: "1207664663".to_owned(),
        kind: EntryKind::Wishlist,
        title: "Witcher 3".to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    };
    StoreEntryRepository(db)
        .upsert_many(std::slice::from_ref(&entry))
        .await
        .expect("entry");
    GameLinkRepository(db)
        .set_manual(&GameLink {
            game_id: game.id,
            store_entry_id: entry.id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await
        .expect("link");
    game.id
}

#[tokio::test]
async fn the_identity_pass_joins_the_family_to_igdb_and_a_gog_wish_is_marked() {
    let lib = library().await;
    // The title of GOG and the title of Steam are not the same text.
    let wish = gog_wish(&lib.db, 1942).await;
    switched_on(&lib, &steam_server().await).await;

    let before = LibraryRepository(&lib.db)
        .one(wish)
        .await
        .expect("row")
        .expect("visible");
    assert_eq!(before.steam_family, None, "no title and no IGDB yet");

    let igdb_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains("\"292030\""))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"[{"id":1,"uid":"292030","game":1942}]"#,
            "application/json",
        ))
        .mount(&igdb_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&igdb_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/game_time_to_beats"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&igdb_server)
        .await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(igdb_server.uri(), format!("{}/token", igdb_server.uri()));
    let credentials = IgdbCredentials {
        client_id: "id".to_owned(),
        client_secret: "secret".to_owned(),
    };
    let token = IgdbToken {
        access_token: "token".to_owned(),
        expires_at: OffsetDateTime::now_utc().unix_timestamp() + 3600,
    };

    let report = resolve(&lib.db, &igdb, &credentials, &token, &Silent)
        .await
        .expect("the identity pass");

    assert_eq!(report.family_joined, 1);
    let after = LibraryRepository(&lib.db)
        .one(wish)
        .await
        .expect("row")
        .expect("visible");
    let mark = after.steam_family.expect("marked by its IGDB record");
    assert_eq!(mark.app_id, "292030");
    assert_eq!(mark.owners, ["Luis"]);
}

#[tokio::test]
async fn the_export_does_not_carry_the_family() {
    let lib = library().await;
    gog_wish(&lib.db, 1942).await;
    switched_on(&lib, &steam_server().await).await;
    SteamFamilyRepository(&lib.db)
        .set_igdb_ids(&std::collections::HashMap::from([(
            "292030".to_owned(),
            1942,
        )]))
        .await
        .expect("join");
    assert!(
        LibraryRepository(&lib.db)
            .all()
            .await
            .expect("library")
            .iter()
            .any(|row| row.steam_family.is_some()),
        "the library shows the mark"
    );

    let dir = tempfile::tempdir().expect("temporary directory");
    let file = dir.path().join("library.json");
    export_library_for(&lib.db, &file, ExportFormat::Json)
        .await
        .expect("export");

    let text = std::fs::read_to_string(file).expect("read");
    assert!(!text.contains("steam_family"), "{text}");
    assert!(!text.contains("Luis"), "the names of the family stay out");
}

#[tokio::test]
async fn the_shared_apps_are_asked_with_the_connected_account() {
    let lib = library().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IFamilyGroupsService/GetFamilyGroupForUser/v1/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GROUP, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/IFamilyGroupsService/GetSharedLibraryApps/v1/"))
        .and(query_param("steamid", USER))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SHARED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/ISteamUser/GetPlayerSummaries/v2/"))
        .and(query_param("key", "TEST_API_KEY"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(NAMES, "application/json"))
        .mount(&server)
        .await;

    enable_family(
        &lib.db,
        &lib.secrets,
        &steam(&server),
        &token_of(USER),
        &Silent,
    )
    .await
    .expect("switch on");

    // `expect(1)` is examined when the server is dropped.
    drop(server);
}
