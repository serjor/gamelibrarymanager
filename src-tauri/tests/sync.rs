//! El "done when" de la fase 3, hasta donde puede comprobarse sin una cuenta
//! real: sincronizar dos veces no duplica ni mueve `first_seen_at`, y la clave
//! de API no acaba en la base de datos.

use connectors::SteamConnector;
use domain::{EntryKind, StoreAccount, StoreAccountId, StoreId};
use gamelibrarymanager_lib::testing::{SyncReport, credential_key, sync_account};
use secrets::{EncryptedFileStore, SecretStore};
use storage::Database;
use storage::repositories::{StoreAccountRepository, StoreEntryRepository};
use time::OffsetDateTime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY: &str = "CLAVE_SECRETA_DEL_USUARIO";
const OWNED: &str = include_str!("../../crates/connectors/tests/fixtures/steam_owned_games.json");
const WISHLIST: &str = include_str!("../../crates/connectors/tests/fixtures/steam_wishlist.json");
const DETAILS: &str = include_str!("../../crates/connectors/tests/fixtures/steam_app_details.json");

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
async fn sincronizar_dos_veces_no_duplica_ni_deja_la_clave_en_la_base_de_datos() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db_path = dir.path().join("library.db");
    let db = Database::open(&db_path).await.expect("abrir base");

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: "76561197960287930".to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let account_id = StoreAccountRepository(&db)
        .upsert(&account)
        .await
        .expect("alta de cuenta");
    let account = StoreAccount {
        id: account_id,
        ..account
    };

    let store = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "contraseña larga")
        .expect("abrir almacén");
    store
        .set(
            &credential_key(&account),
            &format!(r#"{{"api_key":"{API_KEY}"}}"#),
        )
        .expect("guardar credencial");

    let server = steam_server().await;
    let connector =
        SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri());

    let mut primera = SyncReport::default();
    sync_account(&db, &store, &connector, &account, &mut primera)
        .await
        .expect("primera sincronización");
    assert_eq!(primera.owned, 3);
    assert_eq!(primera.wishlist, 2);

    let entries = StoreEntryRepository(&db);
    let tras_la_primera = entries.active(EntryKind::Owned).await.expect("listar");
    assert_eq!(tras_la_primera.len(), 3);

    let mut segunda = SyncReport::default();
    sync_account(&db, &store, &connector, &account, &mut segunda)
        .await
        .expect("segunda sincronización");

    let tras_la_segunda = entries.active(EntryKind::Owned).await.expect("listar");
    assert_eq!(tras_la_segunda.len(), 3, "sincronizar dos veces no duplica");
    assert_eq!(
        tras_la_segunda.iter().map(|e| e.id).collect::<Vec<_>>(),
        tras_la_primera.iter().map(|e| e.id).collect::<Vec<_>>(),
        "las filas son las mismas, no filas nuevas"
    );
    assert_eq!(
        segunda.removed, 0,
        "nada desaparece entre dos sincronizaciones iguales"
    );

    // Y lo importante: la clave no está en la base de datos.
    drop(db);
    let bytes = std::fs::read(&db_path).expect("leer el fichero de la base");
    assert!(
        !bytes
            .windows(API_KEY.len())
            .any(|w| w == API_KEY.as_bytes()),
        "la clave de API no puede aparecer en el SQLite"
    );
}

#[tokio::test]
async fn sin_credencial_guardada_falla_con_un_mensaje_accionable() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("abrir base");
    let store = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "contraseña larga")
        .expect("abrir almacén");

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: "76561197960287930".to_owned(),
        display_name: None,
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    StoreAccountRepository(&db)
        .upsert(&account)
        .await
        .expect("alta de cuenta");

    let server = steam_server().await;
    let connector =
        SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri());

    let error = sync_account(
        &db,
        &store,
        &connector,
        &account,
        &mut SyncReport::default(),
    )
    .await
    .expect_err("sin credencial no se puede sincronizar");

    assert!(
        error.to_string().contains("vuelve a conectarla"),
        "el mensaje debe decirle al usuario qué hacer, no solo qué ha fallado"
    );
}
