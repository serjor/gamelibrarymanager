//! The Steam family against answers of the shape that Steam gives. No test of
//! this suite touches the real network.
//!
//! The fixtures `steam_family_*.json` were written on 2026-10-15 from the
//! fields of `service_familygroups.proto` (SteamTracking/Protobufs) and from the
//! JSON of the Web API, which gives the 64-bit identifiers as text. They are
//! not recorded answers: the service does not accept the API key, and a token
//! of a session is not something to keep in a repository. The live test
//! `src-tauri/tests/live_steam_family.rs` reads the real family of the user.
//! The steamids and the names are invented.

use connectors::SteamConnector;
use connectors::steam::family::{StoreToken, parse_shared_apps};
use domain::{ConnectorError, FamilyGroup, StoreId, StoreSession};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const GROUP: &str = include_str!("fixtures/steam_family_group.json");
const NO_GROUP: &str = include_str!("fixtures/steam_family_no_group.json");
const SHARED: &str = include_str!("fixtures/steam_family_shared_apps.json");
const NAMES: &str = include_str!("fixtures/steam_family_player_summaries.json");

/// The user of the fixtures. They own two of the games of the list.
const USER: &str = "76561197960287930";

fn token() -> StoreToken {
    StoreToken {
        token: "TEST_STORE_TOKEN".to_owned(),
        steam_id: Some(USER.to_owned()),
        expires_at: None,
    }
}

fn connector(server: &MockServer) -> SteamConnector {
    SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri())
}

#[tokio::test]
async fn it_reads_the_family_with_the_token_and_not_with_the_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IFamilyGroupsService/GetFamilyGroupForUser/v1/"))
        .and(query_param("access_token", "TEST_STORE_TOKEN"))
        .and(query_param("steamid", USER))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GROUP, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let group = connector(&server)
        .family_group(&token())
        .await
        .expect("the family");

    assert_eq!(group, FamilyGroup::Member("1234567890123456789".to_owned()));
}

#[tokio::test]
async fn a_user_with_no_family_is_an_answer_and_not_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IFamilyGroupsService/GetFamilyGroupForUser/v1/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(NO_GROUP, "application/json"))
        .mount(&server)
        .await;

    let group = connector(&server)
        .family_group(&token())
        .await
        .expect("an answer");

    assert_eq!(group, FamilyGroup::NotMember);
}

#[tokio::test]
async fn it_asks_only_for_the_games_that_others_can_lend() {
    let server = MockServer::start().await;
    // With no flags, the list has the games of the user, the excluded games
    // and the soundtracks. The parser filters them again, but the request must
    // already say what it wants.
    Mock::given(method("GET"))
        .and(path("/IFamilyGroupsService/GetSharedLibraryApps/v1/"))
        .and(query_param("access_token", "TEST_STORE_TOKEN"))
        .and(query_param("family_groupid", "1234567890123456789"))
        .and(query_param("include_own", "0"))
        .and(query_param("include_excluded", "0"))
        .and(query_param("include_non_games", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SHARED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let apps = connector(&server)
        .shared_apps(&token(), "1234567890123456789", USER)
        .await
        .expect("the games");

    let titles: Vec<&str> = apps.iter().map(|app| app.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Hades", "Disco Elysium", "The Witcher 3: Wild Hunt"]
    );
}

#[test]
fn a_game_that_cannot_be_lent_never_becomes_a_mark() {
    let apps = parse_shared_apps(SHARED, USER).expect("the games");

    // `exclude_reason` 3: in the list, and nobody can lend it.
    assert!(apps.iter().all(|app| app.app_id != "271590"));
    // `app_type` 4: a soundtrack, not a game.
    assert!(apps.iter().all(|app| app.app_id != "1145350"));
    // Only the user owns it.
    assert!(apps.iter().all(|app| app.app_id != "105600"));
}

#[test]
fn the_user_is_not_an_owner_of_what_the_family_lends() {
    let apps = parse_shared_apps(SHARED, USER).expect("the games");

    let witcher = apps
        .iter()
        .find(|app| app.app_id == "292030")
        .expect("a game that another member also owns");
    let owners: Vec<&str> = witcher
        .owners
        .iter()
        .map(|owner| owner.steam_id.as_str())
        .collect();
    assert_eq!(owners, ["76561190000000002"]);

    let disco = apps
        .iter()
        .find(|app| app.app_id == "632470")
        .expect("disco");
    assert_eq!(disco.owners.len(), 2);
}

#[tokio::test]
async fn a_session_that_steam_refuses_is_unauthorised() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IFamilyGroupsService/GetFamilyGroupForUser/v1/"))
        .respond_with(ResponseTemplate::new(401).set_body_string(
            "<html><head><title>Unauthorized</title></head><body><h1>Unauthorized</h1></body></html>",
        ))
        .mount(&server)
        .await;

    let error = connector(&server)
        .family_group(&token())
        .await
        .expect_err("refused");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn the_names_of_the_members_come_with_the_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ISteamUser/GetPlayerSummaries/v2/"))
        .and(query_param("key", "TEST_API_KEY"))
        .and(query_param(
            "steamids",
            "76561190000000001,76561190000000002",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(NAMES, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let session = StoreSession {
        store: StoreId::Steam,
        account_ref: USER.to_owned(),
        display_name: None,
        credential: r#"{"api_key":"TEST_API_KEY"}"#.to_owned(),
        expires_at: None,
    };
    let names = connector(&server)
        .player_names(
            &session,
            &[
                "76561190000000001".to_owned(),
                "76561190000000002".to_owned(),
            ],
        )
        .await
        .expect("the names");

    assert_eq!(
        names.get("76561190000000001").map(String::as_str),
        Some("Ana")
    );
    assert_eq!(
        names.get("76561190000000002").map(String::as_str),
        Some("Luis")
    );
}
