//! Luna against a recorded answer. No test touches the real endpoint: the
//! catalogue rotates each month, thus a test against it could declare nothing.
//!
//! `fixtures/luna_prime_es.json` is the answer of 2026-10-04 for Spain, with
//! `x-amz-locale: en_GB`, cut to five real tiles of the 89. The real tiles
//! include the two prefixes of `gameId`, `amzn1.adg.product` and
//! `amzn1.luna.product`, and Fortnite, which Luna gives with no ASIN. Two
//! widgets were added by hand, and they are the conditions that break: a tile
//! whose action is not the page of a game, and a widget that is not a game.

use metadata::MetadataError;
use metadata::luna::{LunaClient, LunaRegion};
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PRIME_ES: &str = include_str!("fixtures/luna_prime_es.json");

fn client(server: &MockServer) -> LunaClient {
    LunaClient::new(reqwest::Client::new()).with_base(server.uri())
}

fn spain() -> LunaRegion {
    LunaRegion::find("es").expect("Spain is supported")
}

#[tokio::test]
async fn the_prime_catalogue_gives_each_game_with_its_page() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/getPage"))
        // No session: the web page of Luna sends the header empty.
        .and(header("x-amz-access-token", ""))
        // English, whatever the country: the titles of IGDB and of the stores.
        .and(header("x-amz-locale", "en_GB"))
        .and(header("x-amz-marketplace-id", "A1RKKUPIHCS9HS"))
        .and(body_partial_json(serde_json::json!({
            "searchContext": {"query": "included_with_prime"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PRIME_ES, "application/json"))
        .mount(&server)
        .await;

    let catalog = client(&server)
        .prime_catalog(&spain())
        .await
        .expect("catalogue");

    assert_eq!(catalog.territory.as_deref(), Some("ES"));
    let titles: Vec<&str> = catalog.offers.iter().map(|o| o.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Fortnite",
            "Hogwarts Legacy",
            "Ticket to Ride®",
            "Tomb Raider Game of the Year",
            "Batman Caped Crusader - Chronicles",
        ]
    );

    let hogwarts = &catalog.offers[1];
    let asin = hogwarts
        .asin
        .as_deref()
        .expect("Hogwarts Legacy has an ASIN");
    assert_eq!(hogwarts.path, format!("/game/hogwarts-legacy/{asin}"));
    assert!(hogwarts.product_id.starts_with("amzn1.adg.product."));

    // No ASIN is not a reason to lose the game: its page and its identity are
    // still there.
    let fortnite = &catalog.offers[0];
    assert_eq!(fortnite.path, "/game/fortnite");
    assert_eq!(fortnite.asin, None);
    assert!(
        catalog
            .offers
            .iter()
            .any(|o| o.product_id.starts_with("amzn1.luna.product."))
    );
}

#[tokio::test]
async fn a_tile_with_no_page_is_not_given_one() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/getPage"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PRIME_ES, "application/json"))
        .mount(&server)
        .await;

    let catalog = client(&server)
        .prime_catalog(&spain())
        .await
        .expect("catalogue");

    assert!(
        catalog
            .offers
            .iter()
            .all(|o| o.title != "Tile With No Page")
    );
    assert!(catalog.offers.iter().all(|o| o.title != "All games"));
}

#[tokio::test]
async fn an_answer_that_is_not_a_page_is_an_error_and_not_an_empty_catalogue() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/getPage"))
        .respond_with(ResponseTemplate::new(500).set_body_string(
            r#"{"message":"Encountered an unexpected error and could not fulfill the request.","retryable":false}"#,
        ))
        .mount(&server)
        .await;

    // An empty catalogue would delete every mark of the library. An error keeps
    // the catalogue of the last refresh.
    assert!(matches!(
        client(&server).prime_catalog(&spain()).await,
        Err(MetadataError::Unexpected(_))
    ));
}

#[test]
fn each_supported_country_has_its_own_domain_for_the_links() {
    let regions = LunaRegion::supported();
    assert_eq!(regions.len(), 11);
    assert_eq!(
        spain().game_url("/game/hogwarts-legacy/B0CMQK7H1T"),
        "https://luna.amazon.es/game/hogwarts-legacy/B0CMQK7H1T"
    );
    assert!(LunaRegion::find("ZZ").is_none());
}
