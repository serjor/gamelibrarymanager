//! ITAD contra respuestas grabadas. Ningún test toca la API real: haría falta
//! una clave de un usuario, y los precios cambian cada hora, así que un test
//! contra la de verdad no podría afirmar nada.
//!
//! Las fixtures reproducen la forma de la respuesta documentada el 2026-08-15,
//! con los casos que de verdad rompen: un juego sin ninguna oferta y otro que
//! nunca ha estado de rebajas, que es distinto de costar cero.

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
        country: "ES".to_owned(),
    }
}

fn client(server: &MockServer) -> ItadClient {
    ItadClient::new(reqwest::Client::new()).with_base(server.uri())
}

#[tokio::test]
async fn el_appid_de_steam_da_el_juego_de_itad() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/games/lookup/v1"))
        // La clave va en la cabecera y no en la URL: una clave en la dirección
        // acaba en cualquier registro por el que pase la petición.
        .and(header("ITAD-API-Key", "MI_CLAVE"))
        .and(query_param("appid", "632470"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(LOOKUP, "application/json"))
        .mount(&server)
        .await;

    let juego = client(&server)
        .lookup_by_steam_app_id(&credentials(), "632470")
        .await
        .expect("consulta")
        .expect("el juego existe");

    assert_eq!(juego.id, "018d937f-0e3f-72d4-a1a2-6d0e0b0f9d2c");
    assert_eq!(juego.slug, "disco-elysium");
}

#[tokio::test]
async fn un_juego_que_itad_no_conoce_no_es_un_error() {
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
async fn los_precios_llegan_en_centimos_con_el_minimo_historico() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games/prices/v3"))
        .and(query_param("country", "ES"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PRICES, "application/json"))
        .mount(&server)
        .await;

    let precios = client(&server)
        .prices(
            &credentials(),
            &["018d937f-0e3f-72d4-a1a2-6d0e0b0f9d2c".to_owned()],
        )
        .await
        .expect("consulta");

    let disco = &precios[0];
    assert_eq!(disco.deals.len(), 2);
    assert_eq!(disco.deals[0].shop, "Steam");
    assert_eq!(disco.deals[0].price.cents, 1799);
    assert_eq!(disco.deals[0].regular.cents, 3999);
    assert_eq!(disco.deals[0].price.currency, "EUR");
    assert_eq!(disco.deals[0].cut, 55);
    assert_eq!(disco.low_all_time.as_ref().map(|m| m.cents), Some(899));
    assert_eq!(disco.low_year.as_ref().map(|m| m.cents), Some(1349));
}

/// Un juego que no vende nadie y que nunca ha estado de oferta llega con las
/// listas vacías y sin mínimos. No tener precio no es costar cero, y la
/// diferencia se pierde en cuanto se rellena con un valor por defecto.
#[tokio::test]
async fn un_juego_sin_ofertas_llega_sin_precio_y_sin_minimos() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games/prices/v3"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PRICES, "application/json"))
        .mount(&server)
        .await;

    let precios = client(&server)
        .prices(&credentials(), &["da-igual".to_owned()])
        .await
        .expect("consulta");

    let sin_ofertas = &precios[1];
    assert!(sin_ofertas.deals.is_empty());
    assert_eq!(sin_ofertas.low_all_time, None);
    assert_eq!(sin_ofertas.low_year, None);
}

/// Doscientos por petición es lo que admite ITAD. Con una lista de deseados
/// larga, mandarlos todos de golpe devuelve un error y no medio resultado.
#[tokio::test]
async fn una_lista_larga_se_parte_en_lotes_de_doscientos() {
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

    let ids: Vec<String> = (0..450).map(|i| format!("juego-{i}")).collect();
    client(&server)
        .prices(&credentials(), &ids)
        .await
        .expect("consulta");

    assert_eq!(
        server.received_requests().await.expect("peticiones").len(),
        3
    );
}

#[tokio::test]
async fn una_clave_que_no_vale_se_nota_en_la_primera_consulta() {
    let server = MockServer::start().await;
    // ITAD contesta 403, no 401, cuando falta la clave o no sirve.
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
async fn el_429_tiene_su_propio_error() {
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
