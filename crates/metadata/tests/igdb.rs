//! IGDB contra respuestas grabadas. Ningún test toca la API real: además de
//! ser lento y frágil, haría falta una aplicación de Twitch para ejecutarlos.

use metadata::MetadataError;
use metadata::igdb::{IgdbClient, IgdbCredentials, IgdbToken};
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
async fn consigue_un_token_con_las_credenciales_del_usuario() {
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
async fn unas_credenciales_malas_se_notan_al_pedir_el_token() {
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
async fn el_appid_de_steam_da_la_ficha_exacta() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(header("Client-ID", "MI_CLIENT_ID"))
        .and(header("Authorization", "Bearer TOKEN_DE_APLICACION"))
        // Categoría 1 es Steam: sin ese filtro vendrían cruces de otras tiendas.
        .and(body_string_contains("category = 1"))
        .and(body_string_contains("uid = \"632470\""))
        .respond_with(ResponseTemplate::new(200).set_body_raw(EXTERNAL, "application/json"))
        .mount(&server)
        .await;

    let igdb_id = client(&server)
        .by_steam_app_id(&credentials(), &token(), "632470")
        .await
        .expect("consulta");

    assert_eq!(igdb_id, Some(115653));
}

#[tokio::test]
async fn un_appid_desconocido_no_es_un_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    assert_eq!(
        client(&server)
            .by_steam_app_id(&credentials(), &token(), "999999")
            .await
            .expect("consulta"),
        None
    );
}

#[tokio::test]
async fn la_busqueda_devuelve_candidatos_con_nombres_alternativos_y_ano() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SEARCH, "application/json"))
        .mount(&server)
        .await;

    let candidates = client(&server)
        .search(&credentials(), &token(), "Disco Elysium")
        .await
        .expect("búsqueda");

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].igdb_id, 115653);
    assert_eq!(candidates[0].release_year, Some(2019));
    assert_eq!(
        candidates[0].alternative_names,
        vec!["No Truce With The Furies".to_owned()]
    );
}

#[tokio::test]
async fn la_ficha_trae_portada_construida_desde_el_image_id() {
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
        .expect("la ficha existe");

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
async fn el_429_tiene_su_propio_error() {
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
async fn no_se_pasa_de_cuatro_peticiones_por_segundo() {
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
            .search(&credentials(), &token(), &format!("juego {i}"))
            .await
            .expect("búsqueda");
    }

    // Cuatro caben en la primera ventana; las dos siguientes tienen que esperar
    // a que se libere hueco.
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(900),
        "seis peticiones no pueden salir en menos de un segundo"
    );
}
