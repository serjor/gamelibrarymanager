//! Opt-in smoke test with the user's locally stored provider credentials.
//! It only writes to an in-memory database and never prints a secret.

use domain::{Game, GameId, PlatformFamily, matching};
use gamelibrarymanager_lib::testing::{Silent, WishTarget, add_manual_wish_for, refresh_prices};
use metadata::igdb::IgdbCredentials;
use metadata::itad::ItadCredentials;
use metadata::{IgdbClient, ItadClient};
use secrets::{KeyringStore, SecretStore};
use storage::Database;
use storage::repositories::{LibraryRepository, PriceRepository};

const SERVICE: &str = "com.serjor.gamelibrarymanager";

fn game(title: &str) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: matching::normalize(title),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    }
}

#[tokio::test]
#[ignore = "requires locally configured IGDB and ITAD credentials and network access"]
async fn real_providers_accept_manual_pc_and_exclude_console_prices() {
    let secrets = KeyringStore::new(SERVICE);
    let igdb_raw = secrets
        .get("igdb:credentials")
        .expect("read IGDB keyring entry")
        .expect("configure IGDB in the application first");
    let itad_raw = secrets
        .get("itad:credentials")
        .expect("read ITAD keyring entry")
        .expect("configure ITAD in the application first");
    let igdb_credentials: IgdbCredentials =
        serde_json::from_str(&igdb_raw).expect("IGDB credentials format");
    let itad_credentials: ItadCredentials =
        serde_json::from_str(&itad_raw).expect("ITAD credentials format");
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("HTTP client");

    let igdb = IgdbClient::new(http.clone());
    let token = igdb.token(&igdb_credentials).await.expect("Twitch token");
    let found = igdb
        .search(&igdb_credentials, &token, "Hades")
        .await
        .expect("IGDB search");
    let hades = found
        .iter()
        .find(|candidate| candidate.name == "Hades")
        .expect("Hades in IGDB search");
    let detail = igdb
        .game(&igdb_credentials, &token, hades.igdb_id)
        .await
        .expect("IGDB game")
        .expect("Hades details in IGDB");
    assert_eq!(detail.name, "Hades");

    let db = Database::in_memory().await.expect("temporary database");
    let pc = add_manual_wish_for(
        &db,
        WishTarget::New(game("Hades")),
        PlatformFamily::Pc,
        "PC",
    )
    .await
    .expect("manual PC wish");
    let console = add_manual_wish_for(
        &db,
        WishTarget::New(game("Super Mario Odyssey")),
        PlatformFamily::Nintendo,
        "Switch",
    )
    .await
    .expect("manual console wish");
    let targets = PriceRepository(&db).targets().await.expect("price targets");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].game_id, pc.game_id);

    let itad = ItadClient::new(http);
    let report = refresh_prices(&db, &itad, &itad_credentials, &Silent)
        .await
        .expect("ITAD refresh");
    assert_eq!(report.unknown, 0, "ITAD should identify Hades");
    println!(
        "IGDB candidates: {}; ITAD games with active offers: {}",
        found.len(),
        report.priced
    );
    let rows = LibraryRepository(&db).all().await.expect("wishlist rows");
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| row.game_id == pc.game_id && row.manual_wishes.len() == 1)
    );
    assert!(
        rows.iter()
            .any(|row| row.game_id == console.game_id && row.manual_wishes.len() == 1)
    );
    let saved_prices = PriceRepository(&db).all().await.expect("saved prices");
    assert_eq!(saved_prices.len(), report.priced);
    assert!(saved_prices.iter().all(|price| price.game_id == pc.game_id));
}
