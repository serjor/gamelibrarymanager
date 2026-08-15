//! El ciclo completo de la fase 8 contra un ITAD de mentira y una base de datos
//! de verdad: identificar cada deseado, pedir los precios en un solo lote, y no
//! volver a preguntar quién es un juego que ya se sabía.

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, StoreAccount, StoreAccountId, StoreEntry,
    StoreEntryId, StoreId,
};
use gamelibrarymanager_lib::testing::{Silent, refresh_prices};
use metadata::ItadClient;
use metadata::itad::ItadCredentials;
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, PriceRepository, StoreAccountRepository,
    StoreEntryRepository,
};
use time::OffsetDateTime;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DISCO: &str = "018d937f-0e3f-72d4-a1a2-6d0e0b0f9d2c";
const HOLLOW: &str = "018d937f-0e3f-72d4-a1a2-6d0e0b0f9d3a";

fn lookup(id: &str, slug: &str, title: &str) -> String {
    format!(
        r#"{{"found":true,"game":{{"id":"{id}","slug":"{slug}","title":"{title}","type":"game"}}}}"#
    )
}

fn precios(id: &str, tienda: &str, cents: i64, cut: i64) -> String {
    format!(
        r#"{{"id":"{id}",
             "historyLow":{{"all":{{"amount":8.99,"amountInt":899,"currency":"EUR"}},
                            "y1":{{"amount":13.49,"amountInt":1349,"currency":"EUR"}},
                            "m3":null}},
             "deals":[{{"shop":{{"id":61,"name":"{tienda}"}},
                        "price":{{"amount":0.0,"amountInt":{cents},"currency":"EUR"}},
                        "regular":{{"amount":39.99,"amountInt":3999,"currency":"EUR"}},
                        "cut":{cut},
                        "url":"https://store.steampowered.com/app/632470/"}}]}}"#
    )
}

/// Un ITAD que conoce Disco Elysium por su appid y Hollow Knight por su título,
/// y que no conoce nada más.
async fn itad_server() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/games/lookup/v1"))
        .and(query_param("appid", "632470"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            lookup(DISCO, "disco-elysium", "Disco Elysium"),
            "application/json",
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/games/lookup/v1"))
        .and(query_param("title", "Hollow Knight"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            lookup(HOLLOW, "hollow-knight", "Hollow Knight"),
            "application/json",
        ))
        .mount(&server)
        .await;
    // Lo demás no lo conoce, y eso no es un error.
    Mock::given(method("GET"))
        .and(path("/games/lookup/v1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(r#"{"found":false}"#, "application/json"),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/games/prices/v3"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            format!(
                "[{},{}]",
                precios(DISCO, "GOG", 1599, 60),
                precios(HOLLOW, "Steam", 749, 50)
            ),
            "application/json",
        ))
        .mount(&server)
        .await;

    server
}

fn credentials() -> ItadCredentials {
    ItadCredentials {
        key: "clave".to_owned(),
        country: "ES".to_owned(),
    }
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

/// Un deseado con su ficha: una copia de tipo `wishlist` y el enlace que las une.
async fn deseado(db: &Database, store: StoreId, app_id: &str, title: &str) -> GameId {
    let account_id = cuenta(db, store).await;
    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind: EntryKind::Wishlist,
        title: title.to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    };
    StoreEntryRepository(db)
        .upsert_many(std::slice::from_ref(&entry))
        .await
        .expect("volcar entrada");

    let game = Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    GameRepository(db).upsert(&game).await.expect("ficha");

    let mut links = GameLinkRepository(db).all().await.expect("enlaces");
    links.push(GameLink {
        game_id: game.id,
        store_entry_id: entry.id,
        confidence: 1.0,
        method: LinkMethod::Auto,
    });
    GameLinkRepository(db)
        .rebuild_auto(&links)
        .await
        .expect("enlace");

    game.id
}

async fn peticiones(server: &MockServer, ruta: &str) -> usize {
    server
        .received_requests()
        .await
        .expect("peticiones")
        .iter()
        .filter(|peticion| peticion.url.path() == ruta)
        .count()
}

#[tokio::test]
async fn cada_deseado_acaba_con_su_mejor_precio_y_su_minimo_historico() {
    let db = Database::in_memory().await.expect("base");
    let disco = deseado(&db, StoreId::Steam, "632470", "Disco Elysium").await;
    let hollow = deseado(&db, StoreId::Gog, "1207658930", "Hollow Knight").await;

    let server = itad_server().await;
    let itad = ItadClient::new(reqwest::Client::new()).with_base(server.uri());

    let report = refresh_prices(&db, &itad, &credentials(), &Silent)
        .await
        .expect("precios");

    assert_eq!(report.priced, 2);
    assert_eq!(report.unknown, 0);

    let rows = PriceRepository(&db).all().await.expect("consultar precios");
    let de = |game_id: GameId| {
        rows.iter()
            .find(|row| row.game_id == game_id)
            .expect("el juego tiene precio")
    };

    assert_eq!(de(disco).shop, "GOG");
    assert_eq!(de(disco).amount, 1599);
    assert_eq!(de(disco).cut, 60);
    assert_eq!(de(disco).low_all_time, Some(899));
    assert_eq!(de(disco).low_year, Some(1349));
    // El enlace de la ficha sale del slug, que es lo único que la ventana tiene
    // permiso para abrir: la oferta apunta a la tienda que sea.
    assert_eq!(de(disco).itad_slug.as_deref(), Some("disco-elysium"));
    assert_eq!(de(hollow).shop, "Steam");
    assert_eq!(de(hollow).amount, 749);

    // Un solo lote para los dos juegos: una petición por deseado es lo que se
    // come la cuota de una lista larga.
    assert_eq!(peticiones(&server, "/games/prices/v3").await, 1);
}

/// La segunda pasada no vuelve a preguntar quién es cada juego: el
/// identificador quedó anotado en la ficha.
#[tokio::test]
async fn refrescar_dos_veces_no_repite_las_busquedas() {
    let db = Database::in_memory().await.expect("base");
    deseado(&db, StoreId::Steam, "632470", "Disco Elysium").await;

    let server = itad_server().await;
    let itad = ItadClient::new(reqwest::Client::new()).with_base(server.uri());

    for _ in 0..3 {
        refresh_prices(&db, &itad, &credentials(), &Silent)
            .await
            .expect("precios");
    }

    assert_eq!(
        peticiones(&server, "/games/lookup/v1").await,
        1,
        "la búsqueda se hace una vez en la vida del juego"
    );
    assert_eq!(peticiones(&server, "/games/prices/v3").await, 3);

    let rows = PriceRepository(&db).all().await.expect("consultar precios");
    assert_eq!(rows.len(), 1, "refrescar no duplica el precio");
    assert_eq!(rows[0].shops, 1);
}

/// Un juego que ITAD no conoce se cuenta aparte y no arrastra a los demás. Es la
/// misma regla que con las tiendas: lo que falla, falla solo.
#[tokio::test]
async fn un_juego_que_itad_no_conoce_no_deja_sin_precio_a_los_demas() {
    let db = Database::in_memory().await.expect("base");
    deseado(&db, StoreId::Steam, "632470", "Disco Elysium").await;
    deseado(&db, StoreId::Gog, "9999", "Un juego que no existe").await;

    let server = itad_server().await;
    let itad = ItadClient::new(reqwest::Client::new()).with_base(server.uri());

    let report = refresh_prices(&db, &itad, &credentials(), &Silent)
        .await
        .expect("precios");

    assert_eq!(report.unknown, 1);
    assert_eq!(report.priced, 1);
    assert_eq!(
        PriceRepository(&db)
            .all()
            .await
            .expect("consultar precios")
            .len(),
        1
    );
}

/// Comprar un juego lo saca de la lista de deseados, y su precio deja de tener
/// sentido. La siguiente pasada lo olvida.
#[tokio::test]
async fn comprar_un_deseado_borra_su_precio_en_la_siguiente_pasada() {
    let db = Database::in_memory().await.expect("base");
    let disco = deseado(&db, StoreId::Steam, "632470", "Disco Elysium").await;

    let server = itad_server().await;
    let itad = ItadClient::new(reqwest::Client::new()).with_base(server.uri());
    refresh_prices(&db, &itad, &credentials(), &Silent)
        .await
        .expect("precios");
    assert_eq!(PriceRepository(&db).all().await.expect("precios").len(), 1);

    // Deja de estar deseado: la copia se da de baja como haría la sincronización
    // el día que el usuario lo compre.
    let account = StoreAccountRepository(&db)
        .active()
        .await
        .expect("cuentas")
        .into_iter()
        .next()
        .expect("hay cuenta");
    StoreEntryRepository(&db)
        .soft_delete_missing(account.id, EntryKind::Wishlist, &[])
        .await
        .expect("baja");

    refresh_prices(&db, &itad, &credentials(), &Silent)
        .await
        .expect("precios");

    assert!(
        PriceRepository(&db)
            .all()
            .await
            .expect("precios")
            .is_empty(),
        "un juego que ya no se desea no tiene precio que enseñar"
    );
    // La ficha sigue entera: olvidar un precio no toca nada más.
    assert!(
        GameRepository(&db)
            .find(disco)
            .await
            .expect("ficha")
            .is_some()
    );
}
