//! Los dos «done when» de la fase 5 que no son de interfaz: lo que escribe el
//! usuario no lo pisa nadie, y sin red la biblioteca sigue estando ahí.

use connectors::SteamConnector;
use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, PlayStatus, StoreAccount, StoreAccountId,
    StoreId, UserState,
};
use gamelibrarymanager_lib::testing::{SyncReport, credential_key, sync_account};
use secrets::{EncryptedFileStore, SecretStore};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, StoreAccountRepository,
    StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const OWNED: &str = include_str!("../../crates/connectors/tests/fixtures/steam_owned_games.json");
const WISHLIST: &str = include_str!("../../crates/connectors/tests/fixtures/steam_wishlist.json");
const DETAILS: &str = include_str!("../../crates/connectors/tests/fixtures/steam_app_details.json");

async fn escenario(dir: &std::path::Path) -> (Database, EncryptedFileStore, StoreAccount) {
    let db = Database::open(&dir.join("library.db"))
        .await
        .expect("abrir base");

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: "76561197960287930".to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(&db)
        .upsert(&account)
        .await
        .expect("cuenta");
    let account = StoreAccount { id, ..account };

    let store =
        EncryptedFileStore::open(&dir.join("secrets.bin"), "contraseña larga").expect("almacén");
    store
        .set(&credential_key(&account), r#"{"api_key":"CLAVE"}"#)
        .expect("credencial");

    (db, store, account)
}

async fn steam_server() -> MockServer {
    let server = MockServer::start().await;
    for (route, body) in [
        ("/IPlayerService/GetOwnedGames/v1/", OWNED),
        ("/IWishlistService/GetWishlist/v1/", WISHLIST),
        ("/api/appdetails", DETAILS),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .mount(&server)
            .await;
    }
    server
}

#[tokio::test]
async fn el_estado_del_usuario_sobrevive_a_una_sincronizacion_completa() {
    let dir = tempfile::tempdir().expect("temporal");
    let (db, secrets, account) = escenario(dir.path()).await;
    let server = steam_server().await;
    let connector =
        SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri());

    sync_account(
        &db,
        &secrets,
        &connector,
        &account,
        &mut SyncReport::default(),
    )
    .await
    .expect("primera sincronización");

    // Se empareja a mano una de las entradas y el usuario la marca.
    let entrada = StoreEntryRepository(&db)
        .active(EntryKind::Owned)
        .await
        .expect("entradas")
        .into_iter()
        .next()
        .expect("hay entradas");

    let ficha = Game {
        id: GameId::new(),
        canonical_title: "Disco Elysium".to_owned(),
        sort_title: "disco elysium".to_owned(),
        igdb_id: Some(115653),
        cover_url: None,
        summary: None,
        released_at: None,
        genres: vec!["RPG".to_owned()],
    };
    GameRepository(&db).upsert(&ficha).await.expect("ficha");
    GameLinkRepository(&db)
        .set_manual(&GameLink {
            game_id: ficha.id,
            store_entry_id: entrada.id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await
        .expect("enlace");

    UserStateRepository(&db)
        .save(&UserState {
            game_id: ficha.id,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("por el capítulo 3".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("estado");

    // Y ahora se sincroniza otras dos veces, enteras.
    for _ in 0..2 {
        sync_account(
            &db,
            &secrets,
            &connector,
            &account,
            &mut SyncReport::default(),
        )
        .await
        .expect("re-sincronización");
    }

    let rows = LibraryRepository(&db).all().await.expect("biblioteca");
    let row = rows
        .iter()
        .find(|r| r.game_id == ficha.id)
        .expect("la ficha sigue ahí");
    assert_eq!(row.status, Some(PlayStatus::Playing));
    assert_eq!(row.rating, Some(9));
    assert_eq!(row.notes.as_deref(), Some("por el capítulo 3"));
    assert_eq!(row.owned_stores, vec!["steam".to_owned()]);
}

#[tokio::test]
async fn sin_red_la_biblioteca_se_ve_y_lo_que_falla_es_solo_sincronizar() {
    let dir = tempfile::tempdir().expect("temporal");
    let (db, secrets, account) = escenario(dir.path()).await;

    // Se llena la biblioteca con la red disponible.
    let server = steam_server().await;
    let connector =
        SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri());
    sync_account(
        &db,
        &secrets,
        &connector,
        &account,
        &mut SyncReport::default(),
    )
    .await
    .expect("sincronización inicial");

    let entradas = StoreEntryRepository(&db)
        .active(EntryKind::Owned)
        .await
        .expect("entradas");
    let ficha = Game {
        id: GameId::new(),
        canonical_title: "Disco Elysium".to_owned(),
        sort_title: "disco elysium".to_owned(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: vec![],
    };
    GameRepository(&db).upsert(&ficha).await.expect("ficha");
    GameLinkRepository(&db)
        .rebuild_auto(&[GameLink {
            game_id: ficha.id,
            store_entry_id: entradas[0].id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        }])
        .await
        .expect("enlace");

    // Se cae la red: el servidor deja de existir.
    drop(server);
    let caido = SteamConnector::new(reqwest::Client::new())
        .with_bases("http://127.0.0.1:1", "http://127.0.0.1:1");

    let error = sync_account(&db, &secrets, &caido, &account, &mut SyncReport::default())
        .await
        .expect_err("sin red, sincronizar falla");
    assert!(
        error.to_string().contains("no se pudo contactar"),
        "el error debe decir que es la red: {error}"
    );

    // Y aun así la biblioteca se lee entera, que es lo que ve el usuario.
    let rows = LibraryRepository(&db)
        .all()
        .await
        .expect("biblioteca sin red");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Disco Elysium");
}
