//! Opt-in smoke tests with the user's locally stored IGDB credentials.
//! They ask the real `game_time_to_beats` endpoint and never print a secret.

use std::path::PathBuf;

use gamelibrarymanager_lib::testing::refresh_time_to_beat;
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use secrets::{KeyringStore, SecretStore};
use storage::Database;
use storage::repositories::{LibraryRepository, TimeToBeatRepository};
use time::OffsetDateTime;

const SERVICE: &str = "com.serjor.gamelibrarymanager";

/// Hades (2020). A fixed identifier and not a search: IGDB has an older record
/// with the same name and no durations.
const HADES: i64 = 113_112;

async fn igdb() -> (IgdbClient, IgdbCredentials, IgdbToken) {
    let raw = KeyringStore::new(SERVICE)
        .get("igdb:credentials")
        .expect("read IGDB keyring entry")
        .expect("configure IGDB in the application first");
    let credentials: IgdbCredentials = serde_json::from_str(&raw).expect("IGDB credentials format");
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("HTTP client");
    let igdb = IgdbClient::new(http);
    let token = igdb.token(&credentials).await.expect("Twitch token");
    (igdb, credentials, token)
}

/// The path of the open of a record, and the figures that the real endpoint gives.
#[tokio::test]
#[ignore = "requires locally configured IGDB credentials and network access"]
async fn real_igdb_gives_the_time_to_beat_of_a_known_game() {
    let (igdb, credentials, token) = igdb().await;
    let db = Database::in_memory().await.expect("temporary database");

    refresh_time_to_beat(&db, &igdb, &credentials, &token, HADES)
        .await
        .expect("IGDB game_time_to_beats");

    let times = igdb
        .times_to_beat(&credentials, &token, &[HADES])
        .await
        .expect("IGDB game_time_to_beats");
    let time = times.get(&HADES).expect("durations for Hades");
    assert!(time.submissions > 0);
    println!(
        "Hades: hastily {:?} s, normally {:?} s, completely {:?} s, {} players",
        time.hastily, time.normally, time.completely, time.submissions
    );
}

/// The durations step of the pass, on a copy of a real library.
///
/// `TIME_TO_BEAT_DB` names the copy, because the test writes to it. Never point
/// it at the database of the application.
#[tokio::test]
#[ignore = "requires IGDB credentials, network access and TIME_TO_BEAT_DB"]
async fn real_igdb_fills_the_time_to_beat_of_a_library_copy() {
    let path = PathBuf::from(std::env::var("TIME_TO_BEAT_DB").expect("TIME_TO_BEAT_DB"));
    let db = Database::open(&path).await.expect("library copy");
    let (igdb, credentials, token) = igdb().await;

    let repository = TimeToBeatRepository(&db);
    let due = repository
        .due(OffsetDateTime::now_utc())
        .await
        .expect("due records");
    for batch in due.chunks(500) {
        let found = igdb
            .times_to_beat(&credentials, &token, batch)
            .await
            .expect("IGDB game_time_to_beats");
        repository
            .save(batch, &found, OffsetDateTime::now_utc())
            .await
            .expect("save durations");
    }

    let rows = LibraryRepository(&db).all().await.expect("library rows");
    let timed = rows.iter().filter(|row| row.time_to_beat.is_some()).count();
    println!(
        "asked {}; records with durations {timed} of {}",
        due.len(),
        rows.len()
    );
    assert!(timed > 0);
}
