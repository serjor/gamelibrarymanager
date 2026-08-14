//! El "done when" de la fase 6: **un juego que se posee en Steam y en GOG tiene
//! una sola ficha, con las dos insignias de tienda**.
//!
//! Es la primera prueba real de la deduplicación entre tiendas, que es el núcleo
//! del producto, y por eso se comprueba de extremo a extremo: dos cuentas, dos
//! conectores, sincronización, emparejamiento y la consulta que pinta la
//! biblioteca. Ningún test toca la red: todo son respuestas grabadas.
//!
//! El par elegido no es cómodo por casualidad. Steam vende «The Witcher 3: Wild
//! Hunt» y GOG vende «The Witcher 3: Wild Hunt - Complete Edition»: los títulos
//! no coinciden, y que acaben en la misma ficha depende de que la normalización
//! trate «Complete Edition» como empaquetado y no como otro juego.

use connectors::{GogConnector, SteamConnector};
use domain::{StoreAccount, StoreAccountId, StoreId};
use gamelibrarymanager_lib::testing::{Silent, SyncReport, credential_key, resolve, sync_account};
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use secrets::{EncryptedFileStore, SecretStore};
use storage::Database;
use storage::repositories::{LibraryRepository, StoreAccountRepository};
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const STEAM_OWNED: &str =
    include_str!("../../crates/connectors/tests/fixtures/steam_owned_games.json");
const STEAM_WISHLIST: &str =
    include_str!("../../crates/connectors/tests/fixtures/steam_wishlist.json");
const STEAM_DETAILS: &str =
    include_str!("../../crates/connectors/tests/fixtures/steam_app_details.json");

const GOG_RELEASES: &str = include_str!("../../crates/connectors/tests/fixtures/gog_releases.json");
const GOG_RELEASES_2: &str =
    include_str!("../../crates/connectors/tests/fixtures/gog_releases_pagina2.json");
const GOG_PRODUCTS: &str = include_str!("../../crates/connectors/tests/fixtures/gog_products.json");

const IGDB_EXTERNAL: &str = include_str!("fixtures/igdb_external_witcher3.json");
const IGDB_SEARCH: &str = include_str!("fixtures/igdb_search_witcher3.json");
const IGDB_GAME: &str = include_str!("fixtures/igdb_game_witcher3.json");

const STEAM_ID: &str = "76561197960287930";
const GOG_USER_ID: &str = "51000000000000000";
/// El appid de Steam de The Witcher 3, que es el que IGDB sabe cruzar.
const APPID_WITCHER3: &str = "292030";

async fn responde(server: &MockServer, verbo: &str, ruta: &str, cuerpo: &'static str) {
    Mock::given(method(verbo))
        .and(path(ruta))
        .respond_with(ResponseTemplate::new(200).set_body_raw(cuerpo, "application/json"))
        .mount(server)
        .await;
}

async fn servidor_steam() -> MockServer {
    let server = MockServer::start().await;
    for (ruta, cuerpo) in [
        ("/IPlayerService/GetOwnedGames/v1/", STEAM_OWNED),
        ("/IWishlistService/GetWishlist/v1/", STEAM_WISHLIST),
        ("/api/appdetails", STEAM_DETAILS),
    ] {
        responde(&server, "GET", ruta, cuerpo).await;
    }
    server
}

async fn servidor_gog() -> MockServer {
    let server = MockServer::start().await;
    let releases = format!("/users/{GOG_USER_ID}/releases");
    Mock::given(method("GET"))
        .and(path(releases.clone()))
        .and(wiremock::matchers::query_param("page_token", "PAGINA_2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GOG_RELEASES_2, "application/json"))
        .mount(&server)
        .await;
    responde(&server, "GET", &releases, GOG_RELEASES).await;
    responde(&server, "GET", "/products", GOG_PRODUCTS).await;
    server
}

/// IGDB con las dos vías que usa el emparejamiento: el identificador externo
/// para Steam y la búsqueda por nombre para GOG, que no tiene cruce con IGDB.
///
/// Todo lo que no sea The Witcher 3 responde vacío a propósito: así la ficha que
/// aparezca al final solo puede venir de la deduplicación que se quiere probar.
async fn servidor_igdb() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains(APPID_WITCHER3))
        .respond_with(ResponseTemplate::new(200).set_body_raw(IGDB_EXTERNAL, "application/json"))
        .mount(&server)
        .await;
    responde(&server, "POST", "/external_games", "[]").await;

    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("where id = 1942"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(IGDB_GAME, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("search \"The Witcher 3"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(IGDB_SEARCH, "application/json"))
        .mount(&server)
        .await;
    responde(&server, "POST", "/games", "[]").await;

    server
}

fn cuenta(store: StoreId, account_ref: &str) -> StoreAccount {
    StoreAccount {
        id: StoreAccountId::new(),
        store,
        account_ref: account_ref.to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    }
}

#[tokio::test]
async fn un_juego_en_steam_y_en_gog_es_una_sola_ficha_con_dos_insignias() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("abrir base");
    let secretos = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "contraseña larga")
        .expect("abrir almacén");

    // --- dos cuentas, una por tienda ---
    let repo = StoreAccountRepository(&db);
    let mut steam = cuenta(StoreId::Steam, STEAM_ID);
    steam.id = repo.upsert(&steam).await.expect("alta de Steam");
    let mut gog = cuenta(StoreId::Gog, GOG_USER_ID);
    gog.id = repo.upsert(&gog).await.expect("alta de GOG");

    secretos
        .set(&credential_key(&steam), r#"{"api_key":"CLAVE"}"#)
        .expect("credencial de Steam");
    secretos
        .set(&credential_key(&gog), &credencial_gog())
        .expect("credencial de GOG");

    // --- sincronizar las dos ---
    let steam_server = servidor_steam().await;
    let gog_server = servidor_gog().await;

    let conector_steam = SteamConnector::new(reqwest::Client::new())
        .with_bases(steam_server.uri(), steam_server.uri());
    let conector_gog = GogConnector::new(reqwest::Client::new()).with_bases(&gog_server.uri());

    let mut informe = SyncReport::default();
    sync_account(&db, &secretos, &conector_steam, &steam, &mut informe)
        .await
        .expect("sincronizar Steam");
    sync_account(&db, &secretos, &conector_gog, &gog, &mut informe)
        .await
        .expect("sincronizar GOG");

    assert_eq!(informe.owned, 5, "3 copias de Steam y 2 de GOG");

    // --- emparejar contra IGDB ---
    let igdb_server = servidor_igdb().await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(igdb_server.uri(), format!("{}/token", igdb_server.uri()));

    resolve(&db, &igdb, &credenciales_igdb(), &token_igdb(), &Silent)
        .await
        .expect("emparejar");

    // --- y lo que tiene que verse: UNA ficha con DOS insignias ---
    let biblioteca = LibraryRepository(&db).all().await.expect("biblioteca");

    assert_eq!(
        biblioteca.len(),
        1,
        "solo The Witcher 3 tiene ficha; el resto se queda sin emparejar a propósito"
    );

    let witcher = &biblioteca[0];
    assert_eq!(witcher.title, "The Witcher 3: Wild Hunt");
    assert_eq!(
        witcher.owned_stores,
        vec!["gog".to_owned(), "steam".to_owned()],
        "la copia de Steam y la de GOG cuelgan de la misma ficha"
    );
}

#[tokio::test]
async fn volver_a_emparejar_no_desdobla_la_ficha() {
    // La deduplicación tiene que ser idempotente: si al re-emparejar apareciese
    // una segunda ficha, el usuario vería su juego dos veces y perdería de vista
    // el estado que colgaba de la primera.
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("abrir base");
    let secretos = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "contraseña larga")
        .expect("abrir almacén");

    let repo = StoreAccountRepository(&db);
    let mut steam = cuenta(StoreId::Steam, STEAM_ID);
    steam.id = repo.upsert(&steam).await.expect("alta de Steam");
    let mut gog = cuenta(StoreId::Gog, GOG_USER_ID);
    gog.id = repo.upsert(&gog).await.expect("alta de GOG");
    secretos
        .set(&credential_key(&steam), r#"{"api_key":"CLAVE"}"#)
        .expect("credencial de Steam");
    secretos
        .set(&credential_key(&gog), &credencial_gog())
        .expect("credencial de GOG");

    let steam_server = servidor_steam().await;
    let gog_server = servidor_gog().await;
    let igdb_server = servidor_igdb().await;

    let conector_steam = SteamConnector::new(reqwest::Client::new())
        .with_bases(steam_server.uri(), steam_server.uri());
    let conector_gog = GogConnector::new(reqwest::Client::new()).with_bases(&gog_server.uri());
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(igdb_server.uri(), format!("{}/token", igdb_server.uri()));

    for _ in 0..2 {
        let mut informe = SyncReport::default();
        sync_account(&db, &secretos, &conector_steam, &steam, &mut informe)
            .await
            .expect("sincronizar Steam");
        sync_account(&db, &secretos, &conector_gog, &gog, &mut informe)
            .await
            .expect("sincronizar GOG");
        resolve(&db, &igdb, &credenciales_igdb(), &token_igdb(), &Silent)
            .await
            .expect("emparejar");
    }

    let biblioteca = LibraryRepository(&db).all().await.expect("biblioteca");
    assert_eq!(
        biblioteca.len(),
        1,
        "dos pasadas siguen dando una sola ficha"
    );
    assert_eq!(
        biblioteca[0].owned_stores,
        vec!["gog".to_owned(), "steam".to_owned()]
    );
}

fn credencial_gog() -> String {
    let futuro = OffsetDateTime::now_utc().unix_timestamp() + 3600;
    format!(
        r#"{{"client_id":"46899977096215655","client_secret":"SECRETO_DEL_USUARIO",
             "access_token":"ACCESO","refresh_token":"REFRESCO",
             "user_id":"{GOG_USER_ID}","expires_at":{futuro}}}"#
    )
}

fn credenciales_igdb() -> IgdbCredentials {
    IgdbCredentials {
        client_id: "CLIENTE".to_owned(),
        client_secret: "SECRETO".to_owned(),
    }
}

fn token_igdb() -> IgdbToken {
    IgdbToken {
        access_token: "TOKEN".to_owned(),
        expires_at: OffsetDateTime::now_utc().unix_timestamp() + 3600,
    }
}
