//! El ciclo completo de la fase 4 contra un IGDB de mentira y una base de datos
//! de verdad: appid exacto, título dudoso, y la garantía de que re-emparejar no
//! toca lo que el usuario decidió a mano.

use domain::{
    EntryKind, GameLink, LinkMethod, StoreAccount, StoreAccountId, StoreEntry, StoreEntryId,
    StoreId,
};
use gamelibrarymanager_lib::testing::{Silent, resolve};
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, MatchCandidateRepository, StoreAccountRepository,
    StoreEntryRepository,
};
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EXTERNAL: &str = r#"[{"id":1,"uid":"632470","game":115653}]"#;
const GAME_115653: &str = r#"[{"id":115653,"name":"Disco Elysium","first_release_date":1571270400,
                              "cover":{"id":1,"image_id":"co1x2y"}}]"#;
const SEARCH_AMBIGUO: &str = r#"[{"id":250,"name":"Doom","first_release_date":757382400},
                                 {"id":7351,"name":"Doom","first_release_date":1463011200}]"#;

fn credentials() -> IgdbCredentials {
    IgdbCredentials {
        client_id: "id".to_owned(),
        client_secret: "secreto".to_owned(),
    }
}

fn token() -> IgdbToken {
    IgdbToken {
        access_token: "token".to_owned(),
        expires_at: OffsetDateTime::now_utc().unix_timestamp() + 3600,
    }
}

async fn igdb_server(search_body: &'static str) -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains("\"632470\""))
        .respond_with(ResponseTemplate::new(200).set_body_raw(EXTERNAL, "application/json"))
        .mount(&server)
        .await;
    // Cualquier otro appid es desconocido para IGDB.
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    // La ficha concreta y la búsqueda comparten endpoint: las distingue el
    // cuerpo de la consulta.
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("where id = 115653"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GAME_115653, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("search"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(search_body, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    server
}

async fn cuenta(db: &Database, store: StoreId) -> StoreAccountId {
    StoreAccountRepository(db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store,
            account_ref: format!("cuenta-{}", store.as_str()),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("alta de cuenta")
}

fn entrada(account_id: StoreAccountId, store: StoreId, app_id: &str, title: &str) -> StoreEntry {
    StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind: EntryKind::Owned,
        title: title.to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    }
}

#[tokio::test]
async fn el_appid_de_steam_enlaza_sin_preguntar_y_el_titulo_dudoso_va_a_la_cola() {
    let db = Database::in_memory().await.expect("base");
    let steam = cuenta(&db, StoreId::Steam).await;
    let gog = cuenta(&db, StoreId::Gog).await;

    let exacto = entrada(steam, StoreId::Steam, "632470", "Disco Elysium");
    let dudoso = entrada(gog, StoreId::Gog, "1234", "Doom");
    StoreEntryRepository(&db)
        .upsert_many(&[exacto.clone(), dudoso.clone()])
        .await
        .expect("volcar entradas");

    let server = igdb_server(SEARCH_AMBIGUO).await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    let report = resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("emparejar");

    assert_eq!(report.linked, 1, "el appid exacto se enlaza solo");
    assert_eq!(
        report.review, 1,
        "dos Doom con el mismo nombre no se deciden solos"
    );

    let links = GameLinkRepository(&db).all().await.expect("enlaces");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].store_entry_id, exacto.id);
    assert_eq!(
        links[0].confidence, 1.0,
        "un identificador externo no admite grados de confianza"
    );

    // La ficha se creó con los metadatos de IGDB, no con el título de la tienda.
    let games = GameRepository(&db).all().await.expect("fichas");
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].igdb_id, Some(115653));
    assert!(games[0].cover_url.is_some(), "la portada viene de IGDB");

    // Y lo dudoso quedó en la cola con sus candidatos, listo para el usuario.
    let candidatos = MatchCandidateRepository(&db)
        .for_entry(dudoso.id)
        .await
        .expect("candidatos");
    assert_eq!(candidatos.len(), 2);
}

#[tokio::test]
async fn reemparejar_no_altera_ningun_enlace_manual() {
    let db = Database::in_memory().await.expect("base");
    let gog = cuenta(&db, StoreId::Gog).await;
    let dudoso = entrada(gog, StoreId::Gog, "1234", "Doom");
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&dudoso))
        .await
        .expect("volcar entrada");

    let server = igdb_server(SEARCH_AMBIGUO).await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    // Primera pasada: a la cola.
    resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("emparejar");
    assert!(
        GameLinkRepository(&db)
            .all()
            .await
            .expect("enlaces")
            .is_empty()
    );

    // El usuario decide: es el Doom de 2016.
    let elegido = domain::Game {
        id: domain::GameId::new(),
        canonical_title: "Doom".to_owned(),
        sort_title: "doom".to_owned(),
        igdb_id: Some(7351),
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    GameRepository(&db).upsert(&elegido).await.expect("ficha");
    GameLinkRepository(&db)
        .set_manual(&GameLink {
            game_id: elegido.id,
            store_entry_id: dudoso.id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await
        .expect("enlace manual");

    // Se vuelve a emparejar, dos veces más.
    for _ in 0..2 {
        resolve(&db, &igdb, &credentials(), &token(), &Silent)
            .await
            .expect("re-emparejar");
    }

    let links = GameLinkRepository(&db).all().await.expect("enlaces");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].method, LinkMethod::Manual, "sigue siendo manual");
    assert_eq!(
        links[0].game_id, elegido.id,
        "y sigue apuntando a lo que eligió el usuario"
    );
}

/// Un corte de IGDB a mitad no puede tirar la pasada entera.
///
/// Es lo que pasaba: los enlaces se escribían solo al final, así que un 429 en
/// el juego trescientos —cinco minutos de límite de peticiones— dejaba la base
/// exactamente como estaba. La pasada ahora se para donde le corten, guarda lo
/// de atrás y dice por qué.
#[tokio::test]
async fn un_corte_de_igdb_no_tira_lo_que_ya_se_habia_emparejado() {
    let db = Database::in_memory().await.expect("base");
    let steam = cuenta(&db, StoreId::Steam).await;
    let gog = cuenta(&db, StoreId::Gog).await;

    // «Disco Elysium» va antes que «Doom» por título, que es el orden en que
    // llegan: la primera se empareja por appid y la segunda corta la pasada.
    let exacto = entrada(steam, StoreId::Steam, "632470", "Disco Elysium");
    let que_corta = entrada(gog, StoreId::Gog, "1234", "Doom");
    StoreEntryRepository(&db)
        .upsert_many(&[exacto.clone(), que_corta.clone()])
        .await
        .expect("volcar entradas");

    let server = MockServer::start().await;
    // El cruce va por lotes: el appid viaja dentro de `uid = (…)`, no suelto.
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains("\"632470\""))
        .respond_with(ResponseTemplate::new(200).set_body_raw(EXTERNAL, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("where id = 115653"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GAME_115653, "application/json"))
        .mount(&server)
        .await;
    // La búsqueda por título es la que se encuentra el límite.
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("search"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    let report = resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("un corte del proveedor es un resultado, no un error");

    assert_eq!(report.linked, 1);
    assert!(
        report
            .stopped
            .as_deref()
            .is_some_and(|motivo| motivo.contains("límite")),
        "la pasada tiene que decir por qué se paró: {:?}",
        report.stopped
    );

    // Y lo de antes del corte está escrito, que es todo el asunto.
    let links = GameLinkRepository(&db).all().await.expect("enlaces");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].store_entry_id, exacto.id);
}

#[tokio::test]
async fn un_juego_que_igdb_no_conoce_se_cuenta_aparte_y_no_se_inventa_ficha() {
    let db = Database::in_memory().await.expect("base");
    let gog = cuenta(&db, StoreId::Gog).await;
    let raro = entrada(
        gog,
        StoreId::Gog,
        "9999",
        "Un juego que no existe en ningún sitio",
    );
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&raro))
        .await
        .expect("volcar entrada");

    let server = igdb_server("[]").await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    let report = resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("emparejar");

    assert_eq!(report.unknown, 1);
    assert_eq!(report.linked, 0);
    assert!(
        GameRepository(&db).all().await.expect("fichas").is_empty(),
        "sin candidatos no se inventa una ficha: lo decide el usuario"
    );
}

/// GOG y Epic también tienen identificador exacto, y desde que lo tienen no
/// pasan por la búsqueda por título.
///
/// Cada tienda pregunta por su propia fuente de `external_games`: el
/// `external_id` de Galaxy para GOG y la oferta de la tienda para Epic, que
/// viaja en `raw` porque no está en la copia del lanzador.
#[tokio::test]
async fn gog_y_epic_enlazan_por_identificador_y_no_llegan_a_buscar_por_titulo() {
    let db = Database::in_memory().await.expect("base");
    let gog = cuenta(&db, StoreId::Gog).await;
    let epic = cuenta(&db, StoreId::Epic).await;

    let de_gog = entrada(gog, StoreId::Gog, "1207658930", "The Witcher 3");
    let mut de_epic = entrada(epic, StoreId::Epic, "Heron", "Alan Wake");
    de_epic.raw = serde_json::json!({ "offerId": "OFERTA_ALAN_WAKE" });
    StoreEntryRepository(&db)
        .upsert_many(&[de_gog.clone(), de_epic.clone()])
        .await
        .expect("volcar entradas");

    let server = MockServer::start().await;
    for (fuente, cuerpo) in [
        (5, r#"[{"id":1,"uid":"1207658930","game":1942}]"#),
        (26, r#"[{"id":2,"uid":"OFERTA_ALAN_WAKE","game":548}]"#),
    ] {
        Mock::given(method("POST"))
            .and(path("/external_games"))
            .and(body_string_contains(format!(
                "external_game_source = {fuente}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_raw(cuerpo, "application/json"))
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    // Si alguna de las dos copias llegara a la búsqueda por título, esta
    // expectativa de cero llamadas fallaría al soltar el servidor.
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("search"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"[{"id":1942,"name":"The Witcher 3: Wild Hunt"}]"#,
            "application/json",
        ))
        .mount(&server)
        .await;

    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    let report = resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("emparejar");

    assert_eq!(
        report.linked, 2,
        "las dos copias tienen identificador exacto"
    );
    assert_eq!(report.review, 0);

    let links = GameLinkRepository(&db).all().await.expect("enlaces");
    assert_eq!(links.len(), 2);
    assert!(
        links.iter().all(|link| link.confidence == 1.0),
        "un identificador externo no se puntúa: vale 1.0 o no vale"
    );

    drop(server);
}
