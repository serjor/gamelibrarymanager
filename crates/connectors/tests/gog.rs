//! Conector de GOG contra respuestas grabadas. Ningún test de esta suite toca
//! la red real.
//!
//! Las fixtures de `products` y de la forma de la biblioteca se tomaron de las
//! respuestas reales del 2026-08-14, después de comprobar que los endpoints que
//! documentaba el plan —los de `embed.gog.com`— ya solo devuelven una
//! redirección a la pantalla de login.

use connectors::GogConnector;
use domain::{
    AuthContext, ClientCredentials, ConnectorError, StoreAccountId, StoreConnector, StoreId,
    StoreSession,
};
use time::OffsetDateTime;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = include_str!("fixtures/gog_token.json");
const TOKEN_REFRESCADO: &str = include_str!("fixtures/gog_token_refrescado.json");
const RELEASES: &str = include_str!("fixtures/gog_releases.json");
const RELEASES_2: &str = include_str!("fixtures/gog_releases_pagina2.json");
const PRODUCTS: &str = include_str!("fixtures/gog_products.json");
const USER: &str = include_str!("fixtures/gog_user.json");

const USER_ID: &str = "51000000000000000";

fn client() -> ClientCredentials {
    ClientCredentials {
        client_id: "46899977096215655".to_owned(),
        client_secret: "SECRETO_QUE_APORTA_EL_USUARIO".to_owned(),
    }
}

fn connector(server: &MockServer) -> GogConnector {
    GogConnector::new(reqwest::Client::new()).with_bases(&server.uri())
}

/// Credencial ya guardada, con la caducidad que pida el test.
fn credencial(expires_at: i64) -> String {
    format!(
        r#"{{"client_id":"46899977096215655","client_secret":"SECRETO_QUE_APORTA_EL_USUARIO",
             "access_token":"TOKEN_DE_ACCESO_DE_PRUEBA","refresh_token":"TOKEN_DE_REFRESCO_DE_PRUEBA",
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

/// La biblioteca paginada: página 1 con testigo, página 2 sin él.
async fn mock_biblioteca(server: &MockServer) {
    let route = format!("/users/{USER_ID}/releases");
    Mock::given(method("GET"))
        .and(path(route.clone()))
        .and(query_param("page_token", "PAGINA_2"))
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
async fn canjea_el_codigo_por_un_token() {
    let server = MockServer::start().await;
    mock(&server, "/token", TOKEN).await;
    mock(&server, &format!("/users/{USER_ID}"), USER).await;

    let session = connector(&server)
        .authenticate(&AuthContext::AuthCode {
            code: "CODIGO_DE_LA_REDIRECCION".to_owned(),
            client: client(),
        })
        .await
        .expect("canjear el código");

    assert_eq!(session.store, StoreId::Gog);
    assert_eq!(session.account_ref, USER_ID);
    assert_eq!(session.display_name.as_deref(), Some("serjor"));
    assert!(
        session.credential.contains("TOKEN_DE_REFRESCO_DE_PRUEBA"),
        "el token de refresco tiene que guardarse: sin él habría que volver a \
         pasar por el login en cada caducidad"
    );
    assert!(
        session.credential.contains("SECRETO_QUE_APORTA_EL_USUARIO"),
        "las credenciales de cliente viajan con la credencial porque el \
         refresco las necesita"
    );
}

#[tokio::test]
async fn un_codigo_caducado_pide_volver_a_conectar() {
    let server = MockServer::start().await;
    // GOG responde 400 con `invalid_grant` a un código ya usado o caducado.
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
        .expect_err("un código gastado no autentica");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn lee_la_biblioteca_paginada_y_descarta_lo_ajeno() {
    let server = MockServer::start().await;
    mock_biblioteca(&server).await;

    let entries = connector(&server)
        .owned(&session(futuro()), StoreAccountId::new())
        .await
        .expect("leer biblioteca");

    // De las cuatro entradas que hay entre las dos páginas solo dos son juegos
    // de GOG en propiedad: la de Steam la lista Galaxy porque el usuario tiene
    // esa tienda conectada, y la otra no se posee.
    assert_eq!(entries.len(), 2);

    let ids: Vec<&str> = entries.iter().map(|e| e.store_app_id.as_str()).collect();
    assert_eq!(ids, vec!["1495134320", "1207658930"]);
    assert!(
        entries.iter().all(|e| e.store == StoreId::Gog),
        "ninguna entrada puede salir de aquí marcada como de otra tienda"
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

    // La portada y la página de la tienda son lo que permite comparar contra la
    // ficha de IGDB al revisar un emparejamiento dudoso.
    assert_eq!(
        entries[0].store_url.as_deref(),
        Some("https://www.gog.com/game/the_witcher_3_wild_hunt_game_of_the_year_edition_game")
    );
    assert!(
        entries[0]
            .cover_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://images-4.gog-statics.com/")),
        "GOG sirve la imagen sin esquema y hay que completarla: {:?}",
        entries[0].cover_url
    );
}

#[tokio::test]
async fn sin_titulo_la_copia_sigue_llegando() {
    let server = MockServer::start().await;
    let route = format!("/users/{USER_ID}/releases");
    Mock::given(method("GET"))
        .and(path(route.clone()))
        .and(query_param("page_token", "PAGINA_2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(RELEASES_2, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_raw(RELEASES, "application/json"))
        .mount(&server)
        .await;
    // GOG no conoce el título de todo lo que Galaxy lista: quedarse sin la copia
    // por no saber su nombre sería peor que enseñarla con uno provisional.
    Mock::given(method("GET"))
        .and(path("/products"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let entries = connector(&server)
        .owned(&session(futuro()), StoreAccountId::new())
        .await
        .expect("un fallo de títulos no invalida la sincronización");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].title, "GOG 1495134320");
}

#[tokio::test]
async fn con_el_token_vivo_no_gasta_ni_una_peticion() {
    let server = MockServer::start().await;
    // Si el conector pidiese token teniendo uno válido, esta expectativa de
    // cero llamadas fallaría al cerrarse el servidor.
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
        .expect("reconstruir la sesión");

    assert_eq!(session.account_ref, USER_ID);
}

#[tokio::test]
async fn refresca_el_token_cuando_el_de_acceso_caduca() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .and(query_param("grant_type", "refresh_token"))
        .and(query_param("refresh_token", "TOKEN_DE_REFRESCO_DE_PRUEBA"))
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
        session.credential.contains("TOKEN_DE_ACCESO_RENOVADO"),
        "la sesión tiene que salir con el token nuevo"
    );
    assert!(
        session.credential.contains("TOKEN_DE_REFRESCO_ROTADO"),
        "GOG rota el token de refresco: guardar el viejo dejaría la cuenta \
         muerta en la siguiente caducidad"
    );
    assert!(
        session.credential.contains("SECRETO_QUE_APORTA_EL_USUARIO"),
        "el refresco no puede perder las credenciales de cliente"
    );
}

#[tokio::test]
async fn un_refresco_rechazado_pide_volver_a_conectar() {
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
        .expect_err("un refresh_token revocado no se puede salvar solo");

    assert!(matches!(error, ConnectorError::Unauthorized));
}

#[tokio::test]
async fn los_deseados_de_gog_no_son_accesibles_y_no_se_inventan() {
    let server = MockServer::start().await;

    let entries = connector(&server)
        .wishlist(&session(futuro()), StoreAccountId::new())
        .await
        .expect("los deseados no pueden hacer fallar la sincronización");

    assert!(
        entries.is_empty(),
        "GOG no expone los deseados a un token de Galaxy; la lista vacía es \
         honesta y el scraping con la sesión web del usuario no es una opción"
    );
}

fn futuro() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp() + 3600
}

fn pasado() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp() - 10
}
