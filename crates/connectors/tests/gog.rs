//! The GOG connector against recorded answers. No test of this suite touches
//! the real network.
//!
//! The fixtures of `products` and of the shape of the library were taken from
//! the real answers of 2026-08-14, after it was found that the endpoints in the
//! plan — those of `embed.gog.com` — now only give back a redirect to the login
//! screen.

use connectors::GogConnector;
use domain::{
    AuthContext, ClientCredentials, ConnectorError, StoreAccountId, StoreConnector, StoreId,
    StoreSession,
};
use time::OffsetDateTime;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = include_str!("fixtures/gog_token.json");
const TOKEN_REFRESCADO: &str = include_str!("fixtures/gog_token_refreshed.json");
const RELEASES: &str = include_str!("fixtures/gog_releases.json");
const RELEASES_2: &str = include_str!("fixtures/gog_releases_page2.json");
const PRODUCTS: &str = include_str!("fixtures/gog_products.json");
const USER: &str = include_str!("fixtures/gog_user.json");

const USER_ID: &str = "51000000000000000";

fn client() -> ClientCredentials {
    ClientCredentials {
        client_id: "46899977096215655".to_owned(),
        client_secret: "SECRET_THAT_THE_USER_SUPPLIES".to_owned(),
    }
}

fn connector(server: &MockServer) -> GogConnector {
    GogConnector::new(reqwest::Client::new()).with_bases(&server.uri())
}

/// A credential already kept, with the expiry that the test asks for.
fn credencial(expires_at: i64) -> String {
    format!(
        r#"{{"client_id":"46899977096215655","client_secret":"SECRET_THAT_THE_USER_SUPPLIES",
             "access_token":"TEST_ACCESS_TOKEN","refresh_token":"TEST_REFRESH_TOKEN",
             "user_id":"{USER_ID}","expires_at":{expires_at}}}"#
    )
}

fn session(expires_at: i64) -> StoreSession {
    StoreSession {
        store: StoreId::Gog,
        account_ref: USER_ID.to_owned(),
        display_name: Some("serjor".to_owned()),
        credential: credencial(expires_at),
        expires_at: None,
    }
}

async fn mock(server: &MockServer, route: &str, body: &'static str) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(server)
        .await;
}

/// The library in pages: page 1 with a token, page 2 with no token.
async fn mock_biblioteca(server: &MockServer) {
    let route = format!("/users/{USER_ID}/releases");
    Mock::given(method("GET"))
        .and(path(route.clone()))
        .and(query_param("page_token", "PAGE_2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(RELEASES_2, "application/json"))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_raw(RELEASES, "application/json"))
        .mount(server)
        .await;
    mock(server, "/products", PRODUCTS).await;
}

#[tokio::test]
async fn it_exchanges_the_code_for_a_token() {
    let server = MockServer::start().await;
    mock(&server, "/token", TOKEN).await;
    mock(&server, &format!("/users/{USER_ID}"), USER).await;

    let session = connector(&server)
        .authenticate(&AuthContext::AuthCode {
            code: "CODE_OF_THE_REDIRECT".to_owned(),
            client: client(),
        })
        .await
        .expect("exchange the code");

    assert_eq!(session.store, StoreId::Gog);
    assert_eq!(session.account_ref, USER_ID);
    assert_eq!(session.display_name.as_deref(), Some("serjor"));
    assert!(
        session.credential.contains("TEST_REFRESH_TOKEN"),
        "the refresh token must be kept: without it the user would have to go \
         through the login again at each expiry"
    );
    assert!(
        session.credential.contains("SECRET_THAT_THE_USER_SUPPLIES"),
        "the client credentials go with the credential because the refresh \
         needs them"
    );
}

#[tokio::test]
async fn an_expired_code_asks_you_to_connect_again() {
    let server = MockServer::start().await;
    // GOG answers 400 with `invalid_grant` to a code already used or expired.
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_raw(
            r#"{"error":"invalid_grant","error_description":"Code doesn't exist or is invalid for the client"}"#,
            "application/json",
        ))
        .mount(&server)
        .await;

    let error = connector(&server)
        .authenticate(&AuthContext::AuthCode {
            code: "YA_USADO".to_owned(),
            client: client(),
        })
        .await
        .expect_err("a spent code does not authenticate");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn it_reads_the_library_in_pages_and_drops_what_is_not_its_own() {
    let server = MockServer::start().await;
    mock_biblioteca(&server).await;

    let entries = connector(&server)
        .owned(&session(futuro()), StoreAccountId::new())
        .await
        .expect("leer biblioteca");

    // Of the four entries in the two pages only two are owned GOG games: Galaxy
    // lists the Steam one because the user has that store connected, and the
    // other one is not owned.
    assert_eq!(entries.len(), 2);

    let ids: Vec<&str> = entries.iter().map(|e| e.store_app_id.as_str()).collect();
    assert_eq!(ids, vec!["1495134320", "1207658930"]);
    assert!(
        entries.iter().all(|e| e.store == StoreId::Gog),
        "no entry can come out of here marked as from a different store"
    );

    assert_eq!(
        entries[0].title,
        "The Witcher 3: Wild Hunt - Complete Edition"
    );
    assert_eq!(
        entries[1].title,
        "The Witcher 2: Assassins of Kings Enhanced Edition"
    );
    assert!(entries[0].acquired_at.is_some(), "owned_since se conserva");

    // The cover and the store page are what let you compare against the IGDB
    // record when you examine an unsure match.
    assert_eq!(
        entries[0].store_url.as_deref(),
        Some("https://www.gog.com/game/the_witcher_3_wild_hunt_game_of_the_year_edition_game")
    );
    assert!(
        entries[0]
            .cover_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://images-4.gog-statics.com/")),
        "GOG gives the image with no scheme and it must be completed: {:?}",
        entries[0].cover_url
    );
}

#[tokio::test]
async fn with_no_title_the_copy_still_comes() {
    let server = MockServer::start().await;
    let route = format!("/users/{USER_ID}/releases");
    Mock::given(method("GET"))
        .and(path(route.clone()))
        .and(query_param("page_token", "PAGE_2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(RELEASES_2, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_raw(RELEASES, "application/json"))
        .mount(&server)
        .await;
    // GOG does not know the title of each game that Galaxy lists: to lose all of
    // the copy because its name is unknown would be worse than to show it with a
    // temporary name.
    Mock::given(method("GET"))
        .and(path("/products"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let entries = connector(&server)
        .owned(&session(futuro()), StoreAccountId::new())
        .await
        .expect("a failure of the titles does not make the synchronisation invalid");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "GOG 1495134320");
}

#[tokio::test]
async fn with_the_token_still_live_it_uses_no_request() {
    let server = MockServer::start().await;
    // If the connector asked for a token while it had a valid one, this
    // expectation of zero calls would fail when the server closes.
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TOKEN, "application/json"))
        .expect(0)
        .mount(&server)
        .await;

    let session = connector(&server)
        .authenticate(&AuthContext::Stored {
            credential: credencial(futuro()),
        })
        .await
        .expect("build the session again");

    assert_eq!(session.account_ref, USER_ID);
}

#[tokio::test]
async fn it_refreshes_the_token_when_the_access_token_expires() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .and(query_param("grant_type", "refresh_token"))
        .and(query_param("refresh_token", "TEST_REFRESH_TOKEN"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TOKEN_REFRESCADO, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let session = connector(&server)
        .authenticate(&AuthContext::Stored {
            credential: credencial(pasado()),
        })
        .await
        .expect("refrescar el token caducado");

    assert!(
        session.credential.contains("RENEWED_ACCESS_TOKEN"),
        "the session must come out with the new token"
    );
    assert!(
        session.credential.contains("ROTATED_REFRESH_TOKEN"),
        "GOG changes the refresh token: to keep the old one would leave the \
         muerta en la siguiente caducidad"
    );
    assert!(
        session.credential.contains("SECRET_THAT_THE_USER_SUPPLIES"),
        "the refresh cannot lose the client credentials"
    );
}

#[tokio::test]
async fn a_refused_refresh_asks_you_to_connect_again() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&server)
        .await;

    let error = connector(&server)
        .authenticate(&AuthContext::Stored {
            credential: credencial(pasado()),
        })
        .await
        .expect_err("a revoked refresh_token cannot recover alone");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn the_gog_wishlist_is_not_accessible_and_is_not_invented() {
    let server = MockServer::start().await;

    let entries = connector(&server)
        .wishlist(&session(futuro()), StoreAccountId::new())
        .await
        .expect("the wishes cannot make the synchronisation fail");

    assert!(
        entries.is_empty(),
        "GOG does not give the wishes to a Galaxy token; the empty list is \
         honest and a scrape with the web session of the user is not an option"
    );
}

fn futuro() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp() + 3600
}

fn pasado() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp() - 10
}
