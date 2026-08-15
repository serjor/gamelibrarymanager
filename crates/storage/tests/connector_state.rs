//! The two rules of the connector switch that are easy to break by accident:
//! a store nobody has touched is on, and recovering from an error does not turn
//! a connector the user switched off back on.

use domain::StoreId;
use storage::Database;
use storage::repositories::ConnectorStateRepository;

#[tokio::test]
async fn a_store_nobody_has_touched_has_no_row() {
    let db = Database::in_memory().await.expect("open the database");

    let states = ConnectorStateRepository(&db).all().await.expect("read");

    assert!(
        states.is_empty(),
        "the healthy state is not written down: deciding on a first run which \
         stores exist is an answer that changes with every phase"
    );
}

#[tokio::test]
async fn a_store_that_works_never_gets_a_row() {
    // The synchronisation clears the error of every store that went well, which
    // is every store on almost every run. If clearing wrote a row, the absence
    // of a row would stop meaning anything within one pass.
    let db = Database::in_memory().await.expect("open the database");
    let connectors = ConnectorStateRepository(&db);

    connectors
        .record_error(StoreId::Steam, None)
        .await
        .expect("clear the error of a store that has never failed");

    assert!(connectors.all().await.expect("read").is_empty());
}

#[tokio::test]
async fn recovering_from_an_error_does_not_switch_a_connector_back_on() {
    let db = Database::in_memory().await.expect("open the database");
    let connectors = ConnectorStateRepository(&db);

    connectors
        .record_error(StoreId::Epic, Some("credenciales inválidas"))
        .await
        .expect("write the error");
    connectors
        .set_enabled(StoreId::Epic, false)
        .await
        .expect("switch it off");

    // Epic answers again. The error goes, the decision of the user stays.
    connectors
        .record_error(StoreId::Epic, None)
        .await
        .expect("clear the error");

    let states = connectors.all().await.expect("read");
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].store, StoreId::Epic);
    assert_eq!(states[0].last_error, None);
    assert!(
        !states[0].enabled,
        "a store that answers again cannot switch itself back on: turning it \
         off was a decision, not a symptom"
    );
}

#[tokio::test]
async fn switching_a_connector_off_keeps_the_reason_it_was_switched_off_for() {
    let db = Database::in_memory().await.expect("open the database");
    let connectors = ConnectorStateRepository(&db);

    connectors
        .record_error(
            StoreId::Epic,
            Some("Epic ha cambiado su forma de autorizar"),
        )
        .await
        .expect("write the error");
    connectors
        .set_enabled(StoreId::Epic, false)
        .await
        .expect("switch it off");

    let states = connectors.all().await.expect("read");
    assert_eq!(
        states[0].last_error.as_deref(),
        Some("Epic ha cambiado su forma de autorizar"),
        "the reason is what explains the switch a month from now"
    );
}
