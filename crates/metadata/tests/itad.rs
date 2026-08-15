//! ITAD against recorded answers. No test touches the real API: that would need
//! the key of a user, and the prices change each hour, thus a test against the
//! real API could declare nothing.
//!
//! The fixtures repeat the shape of the answer recorded on 2026-08-15, with the
//! conditions that really break: a game with no offer and a game that has never
//! been discounted, which is different from a cost of zero.

use metadata::MetadataError;
use metadata::itad::{ItadClient, ItadCredentials};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const LOOKUP: &str = include_str!("fixtures/itad_lookup.json");
const NOT_FOUND: &str = include_str!("fixtures/itad_lookup_not_found.json");
const PRICES: &str = include_str!("fixtures/itad_prices.json");

fn credentials() -> ItadCredentials {
    ItadCredentials {
        key: "MI_CLAVE".to_owned(),
        country: "GB".to_owned(),
    }
}

fn client(server: &MockServer) -> ItadClient {
    ItadClient::new(reqwest::Client::new()).with_base(server.uri())
}

#[tokio::test]
async fn the_steam_appid_gives_the_itad_game() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/games/lookup/v1"))
        // The key goes in the header and not in the URL: a key in the address
        // goes into each log through which the request passes.
        .and(header("ITAD-API-Key", "MI_CLAVE"))
        .and(query_param("appid", "632470"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(LOOKUP, "application/json"))
        .mount(&server)
        .await;

    let game = client(&server)
        .lookup_by_steam_app_id(&credentials(), "632470")
        .await
        .expect("consulta")
        .expect("the game exists");

    assert_eq!(game.id, "018d937f-0e3f-72d4-a1a2-6d0e0b0f9d2c");
    assert_eq!(game.slug, "disco-elysium");
}

#[tokio::test]
async fn a_game_that_itad_does_not_know_is_not_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/games/lookup/v1"))
        .and(query_param("title", "Juego inventado"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(NOT_FOUND, "application/json"))
        .mount(&server)
        .await;

    assert_eq!(
        client(&server)
            .lookup_by_title(&credentials(), "Juego inventado")
            .await
            .expect("consulta"),
        None
    );
}

#[tokio::test]
async fn the_prices_come_in_cents_with_the_all_time_low() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games/prices/v3"))
        .and(query_param("country", "GB"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PRICES, "application/json"))
        .mount(&server)
        .await;

    let prices = client(&server)
        .prices(
            &credentials(),
            &["018d937f-0e3f-72d4-a1a2-6d0e0b0f9d2c".to_owned()],
        )
        .await
        .expect("consulta");

    let disco = &prices[0];
    assert_eq!(disco.deals.len(), 2);
    assert_eq!(disco.deals[0].shop, "Steam");
    assert_eq!(disco.deals[0].price.cents, 1799);
    assert_eq!(disco.deals[0].regular.cents, 3999);
    assert_eq!(disco.deals[0].price.currency, "EUR");
    assert_eq!(disco.deals[0].cut, 55);
    assert_eq!(disco.low_all_time.as_ref().map(|m| m.cents), Some(899));
    assert_eq!(disco.low_year.as_ref().map(|m| m.cents), Some(1349));
}

/// A game that nobody sells and that has never been on offer comes with empty
/// lists and with no lows. To have no price is not to cost zero, and the
/// difference goes away as soon as you fill it with a default value.
#[tokio::test]
async fn a_game_with_no_offer_comes_with_no_price_and_no_lows() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games/prices/v3"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PRICES, "application/json"))
        .mount(&server)
        .await;

    let prices = client(&server)
        .prices(&credentials(), &["da-igual".to_owned()])
        .await
        .expect("consulta");

    let with_no_offer = &prices[1];
    assert!(with_no_offer.deals.is_empty());
    assert_eq!(with_no_offer.low_all_time, None);
    assert_eq!(with_no_offer.low_year, None);
}

/// Two hundred for each request is what ITAD accepts. With a long wishlist, to
/// send all of them together gives an error and not one half of a result.
#[tokio::test]
async fn a_long_list_is_divided_into_batches_of_two_hundred() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games/prices/v3"))
        .respond_with(|peticion: &Request| {
            let ids: Vec<String> = serde_json::from_slice(&peticion.body).expect("cuerpo");
            assert!(ids.len() <= 200, "ITAD no admite lotes de {}", ids.len());
            ResponseTemplate::new(200).set_body_raw("[]", "application/json")
        })
        .mount(&server)
        .await;

    let ids: Vec<String> = (0..450).map(|i| format!("game-{i}")).collect();
    client(&server)
        .prices(&credentials(), &ids)
        .await
        .expect("consulta");

    assert_eq!(server.received_requests().await.expect("requests").len(), 3);
}

#[tokio::test]
async fn a_key_that_is_not_valid_is_found_at_the_first_query() {
    let server = MockServer::start().await;
    // ITAD answers 403, not 401, when the key is absent or is not usable.
    Mock::given(method("GET"))
        .and(path("/games/lookup/v1"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    assert!(matches!(
        client(&server)
            .lookup_by_title(&credentials(), "Disco Elysium")
            .await,
        Err(MetadataError::Unauthorized)
    ));
}

#[tokio::test]
async fn the_429_has_an_error_of_its_own() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games/prices/v3"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    assert!(matches!(
        client(&server)
            .prices(&credentials(), &["da-igual".to_owned()])
            .await,
        Err(MetadataError::RateLimited)
    ));
}
