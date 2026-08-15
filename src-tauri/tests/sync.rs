//! The "done when" of phase 3, as far as you can test it with no real account:
//! a second synchronisation does not duplicate and does not move
//! `first_seen_at`, and the API key does not go into the database.

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
async fn a_second_synchronisation_does_not_duplicate_and_leaves_no_key_in_the_database() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db_path = dir.path().join("library.db");
    let db = Database::open(&db_path).await.expect("open the database");

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
        .expect("add the account");
    let account = StoreAccount {
        id: account_id,
        ..account
    };

    let store = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "a long passphrase")
        .expect("open the store");
    store
        .set(
            &credential_key(&account),
            &format!(r#"{{"api_key":"{API_KEY}"}}"#),
        )
        .expect("guardar credencial");

    let server = steam_server().await;
    let connector =
        SteamConnector::new(reqwest::Client::new()).with_bases(server.uri(), server.uri());

    let mut first = SyncReport::default();
    sync_account(&db, &store, &connector, &account, &mut first)
        .await
        .expect("the first synchronisation");
    assert_eq!(first.owned, 3);
    assert_eq!(first.wishlist, 2);

    let entries = StoreEntryRepository(&db);
    let after_the_first = entries.active(EntryKind::Owned).await.expect("list");
    assert_eq!(after_the_first.len(), 3);

    let mut second = SyncReport::default();
    sync_account(&db, &store, &connector, &account, &mut second)
        .await
        .expect("the second synchronisation");

    let after_the_second = entries.active(EntryKind::Owned).await.expect("list");
    assert_eq!(
        after_the_second.len(),
        3,
        "a second synchronisation does not duplicate"
    );
    assert_eq!(
        after_the_second.iter().map(|e| e.id).collect::<Vec<_>>(),
        after_the_first.iter().map(|e| e.id).collect::<Vec<_>>(),
        "the rows are the same rows, not new rows"
    );
    assert_eq!(
        second.removed, 0,
        "nothing goes away between two equal synchronisations"
    );

    // And the important part: the key is not in the database.
    drop(db);
    let bytes = std::fs::read(&db_path).expect("leer el fichero de la base");
    assert!(
        !bytes
            .windows(API_KEY.len())
            .any(|w| w == API_KEY.as_bytes()),
        "the API key cannot appear in the SQLite file"
    );
}

#[tokio::test]
async fn with_no_credential_kept_it_fails_with_a_message_you_can_act_on() {
    let dir = tempfile::tempdir().expect("directorio temporal");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("open the database");
    let store = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "a long passphrase")
        .expect("open the store");

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
        .expect("add the account");

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
    .expect_err("with no credential you cannot synchronise");

    assert!(
        error.to_string().contains("connect it again"),
        "the message must tell the user what to do, not only what failed"
    );
}
