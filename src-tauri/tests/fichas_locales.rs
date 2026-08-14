//! Fichas sin IGDB, y lo que pasa cuando IGDB llega después.
//!
//! Bloquear la aplicación entera hasta tener credenciales de Twitch es coherente
//! —la ficha nace del emparejamiento— pero muy duro en el primer arranque. La
//! alternativa tiene un riesgo concreto y es el que se prueba aquí: `user_state`
//! cuelga del `game_id`, así que si al enriquecer una ficha local se creara una
//! ficha nueva, el usuario perdería lo que hubiera escrito encima.

use domain::{EntryKind, GameId};
use domain::{
    PlayStatus, StoreAccount, StoreAccountId, StoreEntry, StoreEntryId, StoreId, UserState,
};
use gamelibrarymanager_lib::testing::{resolve, resolve_local};
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use storage::Database;
use storage::repositories::{
    GameRepository, LibraryRepository, StoreAccountRepository, StoreEntryRepository,
    UserStateRepository,
};
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const IGDB_EXTERNAL: &str = include_str!("fixtures/igdb_external_witcher3.json");
const IGDB_SEARCH: &str = include_str!("fixtures/igdb_search_witcher3.json");
const IGDB_GAME: &str = include_str!("fixtures/igdb_game_witcher3.json");

async fn base() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("abrir base");
    (dir, db)
}

/// Da de alta una cuenta y le cuelga una copia con el título que se le pase.
async fn copia(db: &Database, store: StoreId, app_id: &str, title: &str) -> StoreEntryId {
    let cuenta = StoreAccount {
        id: StoreAccountId::new(),
        store,
        account_ref: format!("cuenta-{}", store.as_str()),
        display_name: None,
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let account_id = StoreAccountRepository(db)
        .upsert(&cuenta)
        .await
        .expect("alta de cuenta");

    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind: EntryKind::Owned,
        title: title.to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        raw: serde_json::Value::Null,
    };
    StoreEntryRepository(db)
        .upsert_many(std::slice::from_ref(&entry))
        .await
        .expect("volcar copia");
    entry.id
}

async fn servidor_igdb() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains("292030"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(IGDB_EXTERNAL, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
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
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
    server
}

fn cliente(server: &MockServer) -> IgdbClient {
    IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()))
}

fn credenciales() -> IgdbCredentials {
    IgdbCredentials {
        client_id: "CLIENTE".to_owned(),
        client_secret: "SECRETO".to_owned(),
    }
}

fn token() -> IgdbToken {
    IgdbToken {
        access_token: "TOKEN".to_owned(),
        expires_at: OffsetDateTime::now_utc().unix_timestamp() + 3600,
    }
}

#[tokio::test]
async fn sin_igdb_la_biblioteca_ya_se_ve_y_deduplica_por_titulo() {
    let (_dir, db) = base().await;
    copia(&db, StoreId::Steam, "292030", "The Witcher 3: Wild Hunt").await;
    copia(
        &db,
        StoreId::Gog,
        "1495134320",
        "The Witcher 3: Wild Hunt - Complete Edition",
    )
    .await;
    copia(&db, StoreId::Steam, "105600", "Terraria").await;

    let informe = resolve_local(&db).await.expect("emparejar sin IGDB");
    assert_eq!(informe.linked, 3);

    let biblioteca = LibraryRepository(&db).all().await.expect("biblioteca");
    assert_eq!(biblioteca.len(), 2, "The Witcher 3 y Terraria");

    let witcher = biblioteca
        .iter()
        .find(|row| row.sort_title.contains("witcher"))
        .expect("la ficha de The Witcher 3");
    assert_eq!(
        witcher.owned_stores,
        vec!["gog".to_owned(), "steam".to_owned()],
        "sin IGDB, la normalización de títulos ya junta las dos tiendas"
    );
    assert_eq!(
        witcher.cover_url, None,
        "una ficha local no se inventa metadatos que no tiene"
    );
}

#[tokio::test]
async fn al_configurar_igdb_la_ficha_se_enriquece_sin_perder_el_estado() {
    let (_dir, db) = base().await;
    copia(&db, StoreId::Steam, "292030", "The Witcher 3: Wild Hunt").await;
    copia(
        &db,
        StoreId::Gog,
        "1495134320",
        "The Witcher 3: Wild Hunt - Complete Edition",
    )
    .await;

    // --- primer arranque, sin IGDB: el usuario ya puede marcar su estado ---
    resolve_local(&db).await.expect("emparejar sin IGDB");
    let biblioteca = LibraryRepository(&db).all().await.expect("biblioteca");
    let ficha_local: GameId = biblioteca[0].game_id;

    UserStateRepository(&db)
        .save(&UserState {
            game_id: ficha_local,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("por el segundo acto".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("guardar estado");

    // --- y más tarde configura IGDB ---
    let server = servidor_igdb().await;
    resolve(&db, &cliente(&server), &credenciales(), &token())
        .await
        .expect("emparejar con IGDB");

    let biblioteca = LibraryRepository(&db).all().await.expect("biblioteca");
    assert_eq!(biblioteca.len(), 1, "sigue habiendo una sola ficha");

    let fila = &biblioteca[0];
    assert_eq!(
        fila.game_id, ficha_local,
        "la ficha se enriquece en su sitio: si naciera otra, el estado del \
         usuario se quedaría colgando de una ficha que ya no se ve"
    );
    assert_eq!(fila.title, "The Witcher 3: Wild Hunt");
    assert!(
        fila.cover_url.is_some(),
        "ahora sí tiene portada, que es lo que aporta IGDB"
    );
    assert_eq!(fila.status, Some(PlayStatus::Playing));
    assert_eq!(fila.rating, Some(9));
    assert_eq!(fila.notes.as_deref(), Some("por el segundo acto"));
    assert_eq!(
        fila.owned_stores,
        vec!["gog".to_owned(), "steam".to_owned()],
        "y las dos tiendas siguen colgando de ella"
    );

    assert_eq!(
        GameRepository(&db).all().await.expect("fichas").len(),
        1,
        "no puede quedar una ficha local huérfana rondando por la base"
    );
}

#[tokio::test]
async fn lo_que_igdb_no_reconoce_no_desaparece_de_la_biblioteca() {
    let (_dir, db) = base().await;
    // Terraria no tiene cruce en las fixtures de IGDB: la búsqueda vuelve vacía.
    copia(&db, StoreId::Steam, "105600", "Terraria").await;

    resolve_local(&db).await.expect("emparejar sin IGDB");
    assert_eq!(
        LibraryRepository(&db)
            .all()
            .await
            .expect("biblioteca")
            .len(),
        1
    );

    let server = servidor_igdb().await;
    resolve(&db, &cliente(&server), &credenciales(), &token())
        .await
        .expect("emparejar con IGDB");

    let biblioteca = LibraryRepository(&db).all().await.expect("biblioteca");
    assert_eq!(
        biblioteca.len(),
        1,
        "que IGDB no lo conozca no es motivo para quitarle al usuario un juego \
         que ya estaba viendo"
    );
    assert_eq!(biblioteca[0].title, "Terraria");
    assert_eq!(biblioteca[0].owned_stores, vec!["steam".to_owned()]);
}
