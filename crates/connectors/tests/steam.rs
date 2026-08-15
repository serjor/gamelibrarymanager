//! The Steam connector against recorded answers. No test of this suite touches
//! the real network: the fixtures are real answers held unchanged.

use connectors::SteamConnector;
use domain::{AuthContext, ConnectorError, StoreAccountId, StoreConnector, StoreId, StoreSession};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const OWNED: &str = include_str!("fixtures/steam_owned_games.json");
const PRIVATE: &str = include_str!("fixtures/steam_owned_private.json");
const WISHLIST: &str = include_str!("fixtures/steam_wishlist.json");
const SUMMARIES: &str = include_str!("fixtures/steam_player_summaries.json");
/// `appdetails` with one appid. With two, the store answers `null` to all of the
/// request, and that answer is also recorded: it is the answer that made each
/// wished-for game come with no name.
const APP_DETAILS: &str = include_str!("fixtures/steam_app_details.json");
const APP_DETAILS_RDR2: &str = include_str!("fixtures/steam_app_details_rdr2.json");
const APP_DETAILS_VARIOS: &str = include_str!("fixtures/steam_app_details_many.json");

fn session() -> StoreSession {
    StoreSession {
        store: StoreId::Steam,
        account_ref: "76561197960287930".to_owned(),
        display_name: Some("serjor".to_owned()),
        credential: r#"{"api_key":"CLAVE_DE_PRUEBA"}"#.to_owned(),
        expires_at: None,
    }
}

fn connector(server: &MockServer) -> SteamConnector {
    SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri())
}

async fn mock(server: &MockServer, route: &str, body: &'static str) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(server)
        .await;
}

#[tokio::test]
async fn it_reads_the_library() {
    let server = MockServer::start().await;
    mock(&server, "/IPlayerService/GetOwnedGames/v1/", OWNED).await;

    let entries = connector(&server)
        .owned(&session(), StoreAccountId::new())
        .await
        .expect("leer biblioteca");

    assert_eq!(entries.len(), 3);
    let disco = &entries[0];
    assert_eq!(disco.store_app_id, "632470");
    assert_eq!(disco.title, "Disco Elysium");
    assert_eq!(disco.playtime_minutes, Some(1240));
    assert_eq!(disco.store, StoreId::Steam);
    // The initial answer is kept so that you can match again without a new
    // request to the store.
    assert_eq!(disco.raw["appid"], 632470);
}

#[tokio::test]
async fn it_asks_for_the_necessary_parameters() {
    let server = MockServer::start().await;
    // With no include_appinfo the names do not come, and with no
    // include_played_free_games the free-to-play games played are absent: the
    // connector must ask for both.
    Mock::given(method("GET"))
        .and(path("/IPlayerService/GetOwnedGames/v1/"))
        .and(query_param("include_appinfo", "1"))
        .and(query_param("include_played_free_games", "1"))
        .and(query_param("steamid", "76561197960287930"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(OWNED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    connector(&server)
        .owned(&session(), StoreAccountId::new())
        .await
        .expect("leer biblioteca");
}

#[tokio::test]
async fn it_tells_a_private_profile_from_an_empty_library() {
    let server = MockServer::start().await;
    mock(&server, "/IPlayerService/GetOwnedGames/v1/", PRIVATE).await;

    let error = connector(&server)
        .owned(&session(), StoreAccountId::new())
        .await
        .expect_err("a private profile is not an empty library");

    assert!(matches!(error, ConnectorError::Private));
}

#[tokio::test]
async fn it_reads_the_wishlist_and_completes_the_titles() {
    let server = MockServer::start().await;
    mock(&server, "/IWishlistService/GetWishlist/v1/", WISHLIST).await;

    // One appid for each request, which is the only form that the store
    // answers. The answer to a request with more than one — `null` — is mounted
    // last, as a catch-all: if the connector asked in batches again, the
    // wished-for games would come with no name and this test would see it.
    Mock::given(method("GET"))
        .and(path("/api/appdetails"))
        .and(query_param("appids", "1145360"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(APP_DETAILS, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/appdetails"))
        .and(query_param("appids", "1174180"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(APP_DETAILS_RDR2, "application/json"))
        .mount(&server)
        .await;
    mock(&server, "/api/appdetails", APP_DETAILS_VARIOS).await;

    let entries = connector(&server)
        .wishlist(&session(), StoreAccountId::new())
        .await
        .expect("leer wishes");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "Hades");
    assert_eq!(entries[1].title, "Red Dead Redemption 2");
    assert!(entries[0].acquired_at.is_some(), "date_added se conserva");

    // And the test examines what is sent, not only what comes back: two appids
    // in the same request is exactly the defect that left 84 wished-for games
    // named "Steam 115800".
    for request in server.received_requests().await.expect("requests") {
        if request.url.path() != "/api/appdetails" {
            continue;
        }
        let appids = request
            .url
            .query_pairs()
            .find(|(clave, _)| clave == "appids")
            .map(|(_, value)| value.to_string())
            .unwrap_or_default();
        assert!(
            !appids.contains(','),
            "appdetails answers `null` to a request with more than one appid, and this carries {appids}"
        );
    }
}

#[tokio::test]
async fn the_wishlist_still_comes_when_the_titles_are_absent() {
    let server = MockServer::start().await;
    mock(&server, "/IWishlistService/GetWishlist/v1/", WISHLIST).await;
    Mock::given(method("GET"))
        .and(path("/api/appdetails"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let entries = connector(&server)
        .wishlist(&session(), StoreAccountId::new())
        .await
        .expect("a failure of the titles does not make the synchronisation invalid");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "Steam 1145360");
}

#[tokio::test]
async fn it_examines_the_key_at_the_connection() {
    let server = MockServer::start().await;
    mock(&server, "/ISteamUser/GetPlayerSummaries/v2/", SUMMARIES).await;

    let session = connector(&server)
        .authenticate(&AuthContext::ApiKey {
            key: "CLAVE_DE_PRUEBA".to_owned(),
            account_ref: "76561197960287930".to_owned(),
        })
        .await
        .expect("autenticar");

    assert_eq!(session.display_name.as_deref(), Some("serjor"));
    assert_eq!(session.store, StoreId::Steam);
    assert!(
        session.credential.contains("CLAVE_DE_PRUEBA"),
        "the key goes inside the opaque block of credentials"
    );
}

#[tokio::test]
async fn an_invalid_key_is_found_at_the_connection() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ISteamUser/GetPlayerSummaries/v2/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let error = connector(&server)
        .authenticate(&AuthContext::ApiKey {
            key: "NO_VALE".to_owned(),
            account_ref: "76561197960287930".to_owned(),
        })
        .await
        .expect_err("403 is an incorrect key");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn the_request_limit_has_an_error_of_its_own() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IPlayerService/GetOwnedGames/v1/"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let error = connector(&server)
        .owned(&session(), StoreAccountId::new())
        .await
        .expect_err("429");

    assert!(matches!(error, ConnectorError::RateLimited));
}
