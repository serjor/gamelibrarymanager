//! Epic connector against recorded answers. No test in this suite touches the
//! real network.
//!
//! The shapes come from two places, both checked on 2026-08-15: the answer of
//! the token endpoint and of `id/api/redirect`, which were asked by hand, and
//! the shape of a catalogue item, which
//! `store-site-backend-static.ak.epicgames.com/freeGamesPromotions` hands over
//! without credentials. The `/home` suffix of `productSlug` and the names of
//! the image types are copied from that answer, not guessed.

use connectors::EpicConnector;
use domain::{
    AuthContext, ClientCredentials, ConnectorError, StoreAccountId, StoreConnector, StoreId,
    StoreSession,
};
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = include_str!("fixtures/epic_token.json");
const TOKEN_REFRESHED: &str = include_str!("fixtures/epic_token_refreshed.json");
const ASSETS: &str = include_str!("fixtures/epic_assets.json");
const ITEM_GAME: &str = include_str!("fixtures/epic_item_game.json");
const ITEM_DLC: &str = include_str!("fixtures/epic_item_dlc.json");
const ITEM_SECOND: &str = include_str!("fixtures/epic_item_second.json");

const ACCOUNT_ID: &str = "a1b2c3d4e5f64788b0c1d2e3f4a5b6c7";
const CLIENT_ID: &str = "34a02cf8f4414e29b15921876da36f9a";
const CLIENT_SECRET: &str = "CLIENT_SECRET_FROM_THE_USER";

/// The pair, base64 encoded, as Epic wants it: in the header and never in the
/// body.
const BASIC: &str =
    "Basic MzRhMDJjZjhmNDQxNGUyOWIxNTkyMTg3NmRhMzZmOWE6Q0xJRU5UX1NFQ1JFVF9GUk9NX1RIRV9VU0VS";

const NAMESPACE_CARDS: &str = "a1b2c3d4e5f6478899aabbccddeeff00";
const NAMESPACE_KENA: &str = "d5241c76f178492ea1540fce45616757";
const ITEM_ID_GAME: &str = "e6ff9d3d4b2a4a5e9b7c0a1d2e3f4a5b";
const ITEM_ID_DLC: &str = "f7e8d9c0b1a2439485766758493021ab";
const ITEM_ID_KENA: &str = "1e8bda5cdbea4b7d81a8c733e2a48f18";

fn client() -> ClientCredentials {
    ClientCredentials {
        client_id: CLIENT_ID.to_owned(),
        client_secret: CLIENT_SECRET.to_owned(),
    }
}

fn connector(server: &MockServer) -> EpicConnector {
    EpicConnector::new(reqwest::Client::new()).with_bases(&server.uri())
}

/// Credential already stored, with whatever expiry the test asks for.
fn credential(expires_at: i64) -> String {
    format!(
        r#"{{"client_id":"{CLIENT_ID}","client_secret":"{CLIENT_SECRET}",
             "access_token":"TEST_ACCESS_TOKEN","refresh_token":"TEST_REFRESH_TOKEN",
             "account_id":"{ACCOUNT_ID}","expires_at":{expires_at}}}"#
    )
}

fn session(expires_at: i64) -> StoreSession {
    StoreSession {
        store: StoreId::Epic,
        account_ref: ACCOUNT_ID.to_owned(),
        display_name: Some("serjor".to_owned()),
        credential: credential(expires_at),
        expires_at: None,
    }
}

async fn mock_token(server: &MockServer, body: &'static str) {
    Mock::given(method("POST"))
        .and(path("/account/api/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(server)
        .await;
}

/// The library: the asset list and one catalogue item per game, which is one
/// request each, the same way legendary asks for them.
async fn mock_library(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/launcher/api/public/assets/Windows"))
        .and(query_param("label", "Live"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(ASSETS, "application/json"))
        .mount(server)
        .await;

    for (namespace, id, body) in [
        (NAMESPACE_CARDS, ITEM_ID_GAME, ITEM_GAME),
        (NAMESPACE_CARDS, ITEM_ID_DLC, ITEM_DLC),
        (NAMESPACE_KENA, ITEM_ID_KENA, ITEM_SECOND),
    ] {
        Mock::given(method("GET"))
            .and(path(format!(
                "/catalog/api/shared/namespace/{namespace}/bulk/items"
            )))
            .and(query_param("id", id))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .mount(server)
            .await;
    }
}

#[tokio::test]
async fn exchanges_the_code_for_a_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/account/api/oauth/token"))
        // The pair travels in the header. Epic does not accept it in the body,
        // and sending it there would leave the secret in any proxy log.
        .and(header("authorization", BASIC))
        .and(body_string_contains("grant_type=authorization_code"))
        // Without `eg1` the answer is a token the launcher services reject, and
        // the library would never arrive.
        .and(body_string_contains("token_type=eg1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TOKEN, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let session = connector(&server)
        .authenticate(&AuthContext::AuthCode {
            code: "CODE_FROM_THE_AUTHORIZATION_PAGE".to_owned(),
            client: client(),
        })
        .await
        .expect("exchange the code");

    assert_eq!(session.store, StoreId::Epic);
    assert_eq!(session.account_ref, ACCOUNT_ID);
    assert_eq!(
        session.display_name.as_deref(),
        Some("serjor"),
        "Epic hands the name back in the token answer: there is no second \
         request to make for it"
    );
    assert!(
        session.credential.contains("TEST_REFRESH_TOKEN"),
        "without the refresh token the user would go through the login again \
         on every expiry"
    );
    assert!(
        session.credential.contains(CLIENT_SECRET),
        "the client credentials travel inside the credential because the \
         refresh needs them"
    );
}

#[tokio::test]
async fn a_spent_code_asks_to_connect_again() {
    let server = MockServer::start().await;
    // Literal answer of Epic on 2026-08-15 to an invented code.
    Mock::given(method("POST"))
        .and(path("/account/api/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_raw(
            r#"{"errorCode":"errors.com.epicgames.account.oauth.authorization_code_not_found",
                "errorMessage":"Sorry the authorization code you supplied was not found.",
                "numericErrorCode":18059,"error":"invalid_grant"}"#,
            "application/json",
        ))
        .mount(&server)
        .await;

    let error = connector(&server)
        .authenticate(&AuthContext::AuthCode {
            code: "ALREADY_USED".to_owned(),
            client: client(),
        })
        .await
        .expect_err("a spent code does not authenticate");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn an_account_with_something_pending_says_where_to_go() {
    // Retrying the login here never works: it is the account that has to do
    // something first, and only Epic knows on which page.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/account/api/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_raw(
            r#"{"errorCode":"errors.com.epicgames.oauth.corrective_action_required",
                "errorMessage":"Corrective action is required to continue",
                "correctiveAction":"DATE_OF_BIRTH",
                "continuationUrl":"https://www.epicgames.com/id/login/continuation?code=x"}"#,
            "application/json",
        ))
        .mount(&server)
        .await;

    let error = connector(&server)
        .authenticate(&AuthContext::AuthCode {
            code: "VALID_BUT_USELESS".to_owned(),
            client: client(),
        })
        .await
        .expect_err("an account with something pending cannot authorise");

    let message = error.to_string();
    assert!(
        message.contains("https://www.epicgames.com/id/login/continuation?code=x"),
        "the message has to carry the page Epic asks to open: {message}"
    );
}

#[tokio::test]
async fn reads_the_library_without_dlc_or_unreal_assets() {
    let server = MockServer::start().await;
    mock_library(&server).await;

    let entries = connector(&server)
        .owned(&session(future()), StoreAccountId::new())
        .await
        .expect("read the library");

    // Of the four assets only two are games: one is a deck pack of the first
    // one, and the other is an Unreal Engine asset of the account.
    assert_eq!(entries.len(), 2);

    let ids: Vec<&str> = entries.iter().map(|e| e.store_app_id.as_str()).collect();
    assert_eq!(ids, vec!["Cardpocalypse", "Snapdragon"]);
    assert!(
        entries.iter().all(|e| e.store == StoreId::Epic),
        "no entry can leave here marked as another store"
    );

    assert_eq!(entries[0].title, "Cardpocalypse");
    assert_eq!(entries[1].title, "Kena: Bridge of Spirits");

    // The cover is what lets a person compare against the IGDB card when the
    // queue asks.
    assert_eq!(
        entries[0].cover_url.as_deref(),
        Some("https://cdn1.epicgames.com/offer/cards_tall-1200x1600")
    );

    // And no page of the store, which is not an oversight. The item does not
    // carry the slug —318 games of a real library, 318 titles, zero slugs— and
    // guessing it from the title lands on the wrong page half the time. In the
    // screen that exists for comparing, that is worse than no link.
    assert!(entries.iter().all(|entry| entry.store_url.is_none()));
}

#[tokio::test]
async fn without_the_catalogue_the_copies_still_arrive() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/launcher/api/public/assets/Windows"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(ASSETS, "application/json"))
        .mount(&server)
        .await;
    // Epic does not know the item of everything the launcher lists, and one
    // failed request out of two hundred cannot cost the whole library.
    Mock::given(method("GET"))
        .and(path(format!(
            "/catalog/api/shared/namespace/{NAMESPACE_CARDS}/bulk/items"
        )))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/catalog/api/shared/namespace/{NAMESPACE_KENA}/bulk/items"
        )))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let entries = connector(&server)
        .owned(&session(future()), StoreAccountId::new())
        .await
        .expect("a catalogue failure does not invalidate the synchronisation");

    // The three assets that are not Unreal come in, the deck pack among them:
    // without its item there is no way to tell it is an add-on. It ends up in
    // the review queue, which is where a person can see it.
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].title, "Epic Cardpocalypse");
}

#[tokio::test]
async fn with_a_live_token_it_does_not_spend_a_request() {
    let server = MockServer::start().await;
    // If the connector asked for a token while holding a valid one, this
    // expectation of zero calls would fail when the server shuts down.
    Mock::given(method("POST"))
        .and(path("/account/api/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TOKEN, "application/json"))
        .expect(0)
        .mount(&server)
        .await;

    let session = connector(&server)
        .authenticate(&AuthContext::Stored {
            credential: credential(future()),
        })
        .await
        .expect("rebuild the session");

    assert_eq!(session.account_ref, ACCOUNT_ID);
}

#[tokio::test]
async fn refreshes_the_token_when_the_access_one_expires() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/account/api/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=TEST_REFRESH_TOKEN"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TOKEN_REFRESHED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let session = connector(&server)
        .authenticate(&AuthContext::Stored {
            credential: credential(past()),
        })
        .await
        .expect("refresh the expired token");

    assert!(
        session.credential.contains("RENEWED_ACCESS_TOKEN"),
        "the session has to come out with the new token"
    );
    assert!(
        session.credential.contains("ROTATED_REFRESH_TOKEN"),
        "Epic rotates the refresh token: keeping the old one would leave the \
         account dead at the next expiry"
    );
    assert!(
        session.credential.contains(CLIENT_SECRET),
        "the refresh cannot lose the client credentials"
    );
}

#[tokio::test]
async fn a_rejected_refresh_asks_to_connect_again() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/account/api/oauth/token"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&server)
        .await;

    let error = connector(&server)
        .authenticate(&AuthContext::Stored {
            credential: credential(past()),
        })
        .await
        .expect_err("a revoked refresh token cannot save itself");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn the_wish_list_of_epic_is_not_reachable_and_is_not_invented() {
    let server = MockServer::start().await;
    mock_token(&server, TOKEN).await;

    let entries = connector(&server)
        .wishlist(&session(future()), StoreAccountId::new())
        .await
        .expect("the wish list cannot make the synchronisation fail");

    assert!(
        entries.is_empty(),
        "Epic keeps the wish list in the GraphQL of its store, which answers to \
         the web session; an empty list is honest and scraping is not an option"
    );
}

fn future() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp() + 3600
}

fn past() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp() - 10
}
