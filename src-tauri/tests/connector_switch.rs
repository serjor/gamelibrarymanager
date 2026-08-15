//! The "done when" of phase 7: forcing an authentication failure leaves the
//! connector in an error state with something the user can act on, and it does
//! not touch Steam or GOG.
//!
//! Epic is the store this was built for. It rests on the private API of its own
//! launcher, so it can stop working on a day nobody chose, and the application
//! has to survive that without losing the rest of the library.

use std::collections::HashMap;
use std::sync::Arc;

use connectors::{EpicConnector, SteamConnector};
use domain::{EntryKind, StoreAccount, StoreAccountId, StoreConnector, StoreId};
use gamelibrarymanager_lib::testing::{Silent, credential_key, sync_stores};
use secrets::{EncryptedFileStore, SecretStore};
use storage::Database;
use storage::repositories::{
    ConnectorStateRepository, StoreAccountRepository, StoreEntryRepository,
};
use time::OffsetDateTime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const OWNED: &str = include_str!("../../crates/connectors/tests/fixtures/steam_owned_games.json");
const WISHLIST: &str = include_str!("../../crates/connectors/tests/fixtures/steam_wishlist.json");
const DETAILS: &str = include_str!("../../crates/connectors/tests/fixtures/steam_app_details.json");

/// A Steam that answers everything it is asked.
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

/// An Epic that has revoked the session, which is what a change on its side
/// looks like from here.
async fn broken_epic_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/account/api/oauth/token"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    server
}

async fn account(db: &Database, store: StoreId, reference: &str) -> StoreAccount {
    let account = StoreAccount {
        id: StoreAccountId::new(),
        store,
        account_ref: reference.to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(db)
        .upsert(&account)
        .await
        .expect("account created");
    StoreAccount { id, ..account }
}

#[tokio::test]
async fn a_broken_epic_leaves_a_reason_behind_and_does_not_touch_steam() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("open the database");
    let secrets = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "contraseña larga")
        .expect("open the store");

    let steam = account(&db, StoreId::Steam, "76561197960287930").await;
    let epic = account(&db, StoreId::Epic, "a1b2c3d4e5f64788b0c1d2e3f4a5b6c7").await;

    secrets
        .set(&credential_key(&steam), r#"{"api_key":"CLAVE"}"#)
        .expect("Steam credential");
    // Already expired, so rebuilding the session goes for a refresh, which is
    // where Epic says no.
    let expired = OffsetDateTime::now_utc().unix_timestamp() - 10;
    secrets
        .set(
            &credential_key(&epic),
            &format!(
                r#"{{"client_id":"34a02cf8f4414e29b15921876da36f9a","client_secret":"SECRETO",
                     "access_token":"VIEJO","refresh_token":"REVOCADO",
                     "account_id":"a1b2c3d4e5f64788b0c1d2e3f4a5b6c7","expires_at":{expired}}}"#
            ),
        )
        .expect("Epic credential");

    let steam_server = steam_server().await;
    let epic_server = broken_epic_server().await;
    let mut connectors: HashMap<StoreId, Arc<dyn StoreConnector>> = HashMap::new();
    connectors.insert(
        StoreId::Steam,
        Arc::new(
            SteamConnector::new(reqwest::Client::new())
                .with_bases(steam_server.uri(), steam_server.uri()),
        ),
    );
    connectors.insert(
        StoreId::Epic,
        Arc::new(EpicConnector::new(reqwest::Client::new()).with_bases(&epic_server.uri())),
    );

    let report = sync_stores(&db, &secrets, &connectors, &Silent)
        .await
        .expect("a broken store cannot bring the synchronisation down");

    // Steam went through. That is the whole point.
    assert_eq!(report.owned, 3);
    assert_eq!(
        StoreEntryRepository(&db)
            .active(EntryKind::Owned)
            .await
            .expect("list")
            .len(),
        3
    );

    // And Epic left a reason that survives the report.
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].store, "epic");

    let states = ConnectorStateRepository(&db).all().await.expect("states");
    let epic_state = states
        .iter()
        .find(|state| state.store == StoreId::Epic)
        .expect("a failed store has to be written down");
    assert!(
        epic_state.enabled,
        "a failure does not switch it off by itself"
    );
    assert!(
        epic_state
            .last_error
            .as_deref()
            .is_some_and(|reason| reason.contains("vuelve a conectar")),
        "the message has to say what to do, not only what failed: {:?}",
        epic_state.last_error
    );
    assert!(
        !states
            .iter()
            .any(|state| state.store == StoreId::Steam && state.last_error.is_some()),
        "the store that went well cannot end up marked"
    );
}

#[tokio::test]
async fn a_switched_off_epic_is_not_even_asked_and_steam_carries_on() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = Database::open(&dir.path().join("library.db"))
        .await
        .expect("open the database");
    let secrets = EncryptedFileStore::open(&dir.path().join("secrets.bin"), "contraseña larga")
        .expect("open the store");

    let steam = account(&db, StoreId::Steam, "76561197960287930").await;
    account(&db, StoreId::Epic, "a1b2c3d4e5f64788b0c1d2e3f4a5b6c7").await;
    secrets
        .set(&credential_key(&steam), r#"{"api_key":"CLAVE"}"#)
        .expect("Steam credential");

    ConnectorStateRepository(&db)
        .set_enabled(StoreId::Epic, false)
        .await
        .expect("switch Epic off");

    let steam_server = steam_server().await;
    // No mock for Epic at all: if the synchronisation asked it anything, the
    // connection would be refused and it would show up as a failure.
    let epic_server = MockServer::start().await;
    let mut connectors: HashMap<StoreId, Arc<dyn StoreConnector>> = HashMap::new();
    connectors.insert(
        StoreId::Steam,
        Arc::new(
            SteamConnector::new(reqwest::Client::new())
                .with_bases(steam_server.uri(), steam_server.uri()),
        ),
    );
    connectors.insert(
        StoreId::Epic,
        Arc::new(EpicConnector::new(reqwest::Client::new()).with_bases(&epic_server.uri())),
    );

    let report = sync_stores(&db, &secrets, &connectors, &Silent)
        .await
        .expect("synchronise");

    assert_eq!(report.owned, 3, "Steam keeps working");
    assert!(report.failures.is_empty(), "what is off does not fail");
    assert_eq!(
        report.skipped,
        vec!["epic".to_owned()],
        "and it is said out loud: a library that quietly stops growing looks \
         like a bug"
    );
    // Epic has no credential stored either. Without the switch that alone would
    // already be a failure on every single run.
    assert_eq!(
        epic_server
            .received_requests()
            .await
            .unwrap_or_default()
            .len(),
        0
    );
}
