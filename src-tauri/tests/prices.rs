//! The complete cycle of phase 8 against a pretend ITAD and a real database: to
//! identify each wished-for game, to ask for the prices in one batch, and not to
//! ask again who a game is when that is already known.

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

fn prices(id: &str, store: &str, cents: i64, cut: i64) -> String {
    format!(
        r#"{{"id":"{id}",
             "historyLow":{{"all":{{"amount":8.99,"amountInt":899,"currency":"EUR"}},
                            "y1":{{"amount":13.49,"amountInt":1349,"currency":"EUR"}},
                            "m3":null}},
             "deals":[{{"shop":{{"id":61,"name":"{store}"}},
                        "price":{{"amount":0.0,"amountInt":{cents},"currency":"EUR"}},
                        "regular":{{"amount":39.99,"amountInt":3999,"currency":"EUR"}},
                        "cut":{cut},
                        "url":"https://store.steampowered.com/app/632470/"}}]}}"#
    )
}

/// An ITAD that knows Disco Elysium by its appid and Hollow Knight by its title,
/// and that knows nothing else.
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
    // It does not know the others, and that is not an error.
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
                prices(DISCO, "GOG", 1599, 60),
                prices(HOLLOW, "Steam", 749, 50)
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
        country: "GB".to_owned(),
    }
}

async fn account(db: &Database, store: StoreId) -> StoreAccountId {
    StoreAccountRepository(db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store,
            account_ref: format!("account-{}", store.as_str()),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("add the account")
}

/// A wished-for game with its record: a copy of the `wishlist` kind and the link
/// that joins them.
async fn wished(db: &Database, store: StoreId, app_id: &str, title: &str) -> GameId {
    let account_id = account(db, store).await;
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
        .expect("write the entry");

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
    GameRepository(db).upsert(&game).await.expect("record");

    let mut links = GameLinkRepository(db).all().await.expect("links");
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

async fn requests(server: &MockServer, ruta: &str) -> usize {
    server
        .received_requests()
        .await
        .expect("requests")
        .iter()
        .filter(|peticion| peticion.url.path() == ruta)
        .count()
}

#[tokio::test]
async fn each_wished_for_game_ends_with_its_best_price_and_its_all_time_low() {
    let db = Database::in_memory().await.expect("database");
    let disco = wished(&db, StoreId::Steam, "632470", "Disco Elysium").await;
    let hollow = wished(&db, StoreId::Gog, "1207658930", "Hollow Knight").await;

    let server = itad_server().await;
    let itad = ItadClient::new(reqwest::Client::new()).with_base(server.uri());

    let report = refresh_prices(&db, &itad, &credentials(), &Silent)
        .await
        .expect("prices");

    assert_eq!(report.priced, 2);
    assert_eq!(report.unknown, 0);

    let rows = PriceRepository(&db).all().await.expect("consultar prices");
    let de = |game_id: GameId| {
        rows.iter()
            .find(|row| row.game_id == game_id)
            .expect("el game tiene price")
    };

    assert_eq!(de(disco).shop, "GOG");
    assert_eq!(de(disco).amount, 1599);
    assert_eq!(de(disco).cut, 60);
    assert_eq!(de(disco).low_all_time, Some(899));
    assert_eq!(de(disco).low_year, Some(1349));
    // The link of the record comes from the slug, which is the only address that
    // the window has permission to open: the offer points to any store.
    assert_eq!(de(disco).itad_slug.as_deref(), Some("disco-elysium"));
    assert_eq!(de(hollow).shop, "Steam");
    assert_eq!(de(hollow).amount, 749);

    // One batch for the two games: one request for each wished-for game is what
    // uses all of the quota of a long list.
    assert_eq!(requests(&server, "/games/prices/v3").await, 1);
}

/// The second pass does not ask again who each game is: the identifier was
/// written in the record.
#[tokio::test]
async fn a_second_refresh_does_not_repeat_the_searches() {
    let db = Database::in_memory().await.expect("database");
    wished(&db, StoreId::Steam, "632470", "Disco Elysium").await;

    let server = itad_server().await;
    let itad = ItadClient::new(reqwest::Client::new()).with_base(server.uri());

    for _ in 0..3 {
        refresh_prices(&db, &itad, &credentials(), &Silent)
            .await
            .expect("prices");
    }

    assert_eq!(
        requests(&server, "/games/lookup/v1").await,
        1,
        "the search is made one time in the life of the game"
    );
    assert_eq!(requests(&server, "/games/prices/v3").await, 3);

    let rows = PriceRepository(&db).all().await.expect("consultar prices");
    assert_eq!(rows.len(), 1, "a refresh does not duplicate the price");
    assert_eq!(rows[0].shops, 1);
}

/// A game that ITAD does not know is counted apart and does not affect the
/// others. It is the same rule as with the stores: what fails, fails alone.
#[tokio::test]
async fn a_game_that_itad_does_not_know_does_not_leave_the_others_with_no_price() {
    let db = Database::in_memory().await.expect("database");
    wished(&db, StoreId::Steam, "632470", "Disco Elysium").await;
    wished(&db, StoreId::Gog, "9999", "A game that does not exist").await;

    let server = itad_server().await;
    let itad = ItadClient::new(reqwest::Client::new()).with_base(server.uri());

    let report = refresh_prices(&db, &itad, &credentials(), &Silent)
        .await
        .expect("prices");

    assert_eq!(report.unknown, 1);
    assert_eq!(report.priced, 1);
    assert_eq!(
        PriceRepository(&db)
            .all()
            .await
            .expect("consultar prices")
            .len(),
        1
    );
}

/// To buy a game takes it out of the wishlist, and its price stops having a
/// meaning. The next pass forgets it.
#[tokio::test]
async fn to_buy_a_wished_for_game_deletes_its_price_at_the_next_pass() {
    let db = Database::in_memory().await.expect("database");
    let disco = wished(&db, StoreId::Steam, "632470", "Disco Elysium").await;

    let server = itad_server().await;
    let itad = ItadClient::new(reqwest::Client::new()).with_base(server.uri());
    refresh_prices(&db, &itad, &credentials(), &Silent)
        .await
        .expect("prices");
    assert_eq!(PriceRepository(&db).all().await.expect("prices").len(), 1);

    // It stops being wished for: the copy is deleted logically as the
    // synchronisation would do on the day that the user buys it.
    let account = StoreAccountRepository(&db)
        .active()
        .await
        .expect("cuentas")
        .into_iter()
        .next()
        .expect("there is an account");
    StoreEntryRepository(&db)
        .soft_delete_missing(account.id, EntryKind::Wishlist, &[])
        .await
        .expect("baja");

    refresh_prices(&db, &itad, &credentials(), &Silent)
        .await
        .expect("prices");

    assert!(
        PriceRepository(&db).all().await.expect("prices").is_empty(),
        "a game that is no longer wished for has no price to show"
    );
    // The record stays complete: to forget a price touches nothing else.
    assert!(
        GameRepository(&db)
            .find(disco)
            .await
            .expect("record")
            .is_some()
    );
}
