//! Conector de Steam contra respuestas grabadas. Ningún test de esta suite
//! toca la red real: las fixtures son respuestas reales congeladas.

use connectors::SteamConnector;
use domain::{AuthContext, ConnectorError, StoreAccountId, StoreConnector, StoreId, StoreSession};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const OWNED: &str = include_str!("fixtures/steam_owned_games.json");
const PRIVATE: &str = include_str!("fixtures/steam_owned_private.json");
const WISHLIST: &str = include_str!("fixtures/steam_wishlist.json");
const SUMMARIES: &str = include_str!("fixtures/steam_player_summaries.json");
const APP_DETAILS: &str = include_str!("fixtures/steam_app_details.json");

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
async fn lee_la_biblioteca() {
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
    // La respuesta original se conserva para poder re-emparejar sin volver a
    // preguntar a la tienda.
    assert_eq!(disco.raw["appid"], 632470);
}

#[tokio::test]
async fn pide_los_parametros_que_hacen_falta() {
    let server = MockServer::start().await;
    // Sin include_appinfo no vienen los nombres, y sin include_played_free_games
    // faltan los free-to-play jugados: el conector debe pedir ambos.
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
async fn distingue_un_perfil_privado_de_una_biblioteca_vacia() {
    let server = MockServer::start().await;
    mock(&server, "/IPlayerService/GetOwnedGames/v1/", PRIVATE).await;

    let error = connector(&server)
        .owned(&session(), StoreAccountId::new())
        .await
        .expect_err("un perfil privado no es una biblioteca vacía");

    assert!(matches!(error, ConnectorError::Private));
}

#[tokio::test]
async fn lee_los_deseados_y_completa_los_titulos() {
    let server = MockServer::start().await;
    mock(&server, "/IWishlistService/GetWishlist/v1/", WISHLIST).await;
    mock(&server, "/api/appdetails", APP_DETAILS).await;

    let entries = connector(&server)
        .wishlist(&session(), StoreAccountId::new())
        .await
        .expect("leer deseados");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "Hades");
    assert_eq!(entries[1].title, "Red Dead Redemption 2");
    assert!(entries[0].acquired_at.is_some(), "date_added se conserva");
}

#[tokio::test]
async fn los_deseados_siguen_llegando_aunque_falten_los_titulos() {
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
        .expect("un fallo de títulos no invalida la sincronización");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "Steam 1145360");
}

#[tokio::test]
async fn valida_la_clave_al_conectar() {
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
        "la clave viaja dentro del bloque opaco de credenciales"
    );
}

#[tokio::test]
async fn una_clave_invalida_se_detecta_al_conectar() {
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
        .expect_err("403 es una clave mala");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn el_limite_de_peticiones_tiene_su_propio_error() {
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
