//! IGDB against recorded answers. No test touches the real API: as well as slow
//! and fragile, that would need a Twitch application to run the tests.

use metadata::MetadataError;
use metadata::igdb::{ExternalSource, IgdbClient, IgdbCredentials, IgdbToken};
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = include_str!("fixtures/igdb_token.json");
const EXTERNAL: &str = include_str!("fixtures/igdb_external_games.json");
const SEARCH: &str = include_str!("fixtures/igdb_search.json");
const GAME: &str = include_str!("fixtures/igdb_game.json");

fn credentials() -> IgdbCredentials {
    IgdbCredentials {
        client_id: "MI_CLIENT_ID".to_owned(),
        client_secret: "MI_SECRETO".to_owned(),
    }
}

fn token() -> IgdbToken {
    IgdbToken {
        access_token: "TOKEN_DE_APLICACION".to_owned(),
        expires_at: OffsetDateTime::now_utc().unix_timestamp() + 3600,
    }
}

fn client(server: &MockServer) -> IgdbClient {
    IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/oauth2/token", server.uri()))
}

#[tokio::test]
async fn it_gets_a_token_with_the_credentials_of_the_user() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TOKEN, "application/json"))
        .mount(&server)
        .await;

    let token = client(&server).token(&credentials()).await.expect("token");

    assert_eq!(token.access_token, "TOKEN_DE_APLICACION");
    assert!(token.is_valid(OffsetDateTime::now_utc()));
}

#[tokio::test]
async fn incorrect_credentials_are_found_when_the_token_is_requested() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    assert!(matches!(
        client(&server).token(&credentials()).await,
        Err(MetadataError::Unauthorized)
    ));
}

#[tokio::test]
async fn the_steam_appid_gives_the_exact_record() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(header("Client-ID", "MI_CLIENT_ID"))
        .and(header("Authorization", "Bearer TOKEN_DE_APLICACION"))
        // Source 1 is Steam: with no such filter, joins of other stores would
        // come. And it is `external_game_source`, not `category`, which IGDB
        // marks as obsolete.
        .and(body_string_contains("external_game_source = 1"))
        .and(body_string_contains("uid = (\"632470\")"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(EXTERNAL, "application/json"))
        .mount(&server)
        .await;

    let cruces = client(&server)
        .by_external_ids(
            &credentials(),
            &token(),
            ExternalSource::Steam,
            &["632470".to_owned()],
        )
        .await
        .expect("consulta");

    assert_eq!(cruces.get("632470"), Some(&115653));
}

#[tokio::test]
async fn each_store_asks_for_its_own_source() {
    let server = MockServer::start().await;
    // GOG is source 5 and Epic is source 26. A question to the incorrect source
    // would give back the record of a different game that shares an identifier.
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains("external_game_source = 5"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(EXTERNAL, "application/json"))
        .mount(&server)
        .await;

    let cruces = client(&server)
        .by_external_ids(
            &credentials(),
            &token(),
            ExternalSource::Gog,
            &["632470".to_owned()],
        )
        .await
        .expect("consulta");

    assert_eq!(cruces.len(), 1);
}

#[tokio::test]
async fn an_unknown_appid_is_not_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    assert!(
        client(&server)
            .by_external_ids(
                &credentials(),
                &token(),
                ExternalSource::Steam,
                &["999999".to_owned()],
            )
            .await
            .expect("consulta")
            .is_empty()
    );
}

/// The batch is what turns a large library into two requests. If
/// somebody asks one copy at a time again, this test says so.
#[tokio::test]
async fn one_thousand_identifiers_fit_in_two_requests() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .expect(2)
        .mount(&server)
        .await;

    let uids: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
    client(&server)
        .by_external_ids(&credentials(), &token(), ExternalSource::Steam, &uids)
        .await
        .expect("consulta");

    // `expect(2)` is examined when the server is dropped.
    drop(server);
}

#[tokio::test]
async fn the_search_gives_candidates_with_alternative_names_and_a_year() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SEARCH, "application/json"))
        .mount(&server)
        .await;

    let candidates = client(&server)
        .search(&credentials(), &token(), "Disco Elysium")
        .await
        .expect("search");

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].igdb_id, 115653);
    assert_eq!(candidates[0].release_year, Some(2019));
    assert_eq!(
        candidates[0].alternative_names,
        vec!["No Truce With The Furies".to_owned()]
    );
}

#[tokio::test]
async fn the_record_carries_a_cover_built_from_the_image_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GAME, "application/json"))
        .mount(&server)
        .await;

    let game = client(&server)
        .game(&credentials(), &token(), 115653)
        .await
        .expect("consulta")
        .expect("the record exists");

    assert_eq!(game.name, "Disco Elysium");
    assert_eq!(
        game.cover_url.as_deref(),
        Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1x2y.jpg")
    );
    assert!(game.summary.is_some());
    assert_eq!(
        game.genres,
        vec!["Role-playing (RPG)".to_owned(), "Adventure".to_owned()]
    );
}

#[tokio::test]
async fn the_429_has_an_error_of_its_own() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    assert!(matches!(
        client(&server)
            .search(&credentials(), &token(), "loquesea")
            .await,
        Err(MetadataError::RateLimited)
    ));
}

#[tokio::test]
async fn it_does_not_go_over_four_requests_each_second() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SEARCH, "application/json"))
        .mount(&server)
        .await;

    let client = client(&server);
    let started = std::time::Instant::now();
    for i in 0..6 {
        client
            .search(&credentials(), &token(), &format!("game {i}"))
            .await
            .expect("search");
    }

    // Four fit in the first window; the next two must wait until a slot
    // becomes free.
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(900),
        "six requests cannot go out in less than one second"
    );
}
