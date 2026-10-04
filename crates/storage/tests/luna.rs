//! The Luna catalogue is a cache that a refresh replaces complete. What these
//! tests watch is that a game that left the catalogue goes, that nothing of
//! Luna reaches a record or what the user wrote, and that the library marks a
//! record by its title when it reads.

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, LunaCatalog, LunaOffer, PlayStatus,
    StoreAccount, StoreAccountId, StoreEntry, StoreEntryId, StoreId, UserState,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, LunaRepository, LunaSettings,
    StoreAccountRepository, StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;

fn offer(product_id: &str, title: &str, asin: Option<&str>) -> LunaOffer {
    let slug = title.to_lowercase().replace(' ', "-");
    LunaOffer {
        product_id: product_id.to_owned(),
        title: title.to_owned(),
        path: match asin {
            Some(asin) => format!("/game/{slug}/{asin}"),
            None => format!("/game/{slug}"),
        },
        asin: asin.map(str::to_owned),
    }
}

fn catalog(offers: Vec<LunaOffer>) -> LunaCatalog {
    LunaCatalog {
        territory: Some("ES".to_owned()),
        offers,
    }
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a valid moment")
}

#[tokio::test]
async fn with_no_country_luna_is_switched_off() {
    let db = Database::in_memory().await.expect("database");
    let luna = LunaRepository(&db);

    assert_eq!(luna.settings().await.expect("settings"), None);

    luna.set_country("ES", "luna.amazon.es")
        .await
        .expect("switch on");
    assert_eq!(
        luna.settings().await.expect("settings"),
        Some(LunaSettings {
            country: "ES".to_owned(),
            site: "luna.amazon.es".to_owned(),
            territory: None,
            refreshed_at: None,
        })
    );
}

#[tokio::test]
async fn a_refresh_replaces_the_catalogue_and_a_game_that_left_goes() {
    let db = Database::in_memory().await.expect("database");
    let luna = LunaRepository(&db);
    luna.set_country("ES", "luna.amazon.es")
        .await
        .expect("switch on");

    luna.replace_catalog(
        &catalog(vec![
            offer("amzn1.adg.product.a", "Hogwarts Legacy", Some("B0CMQK7H1T")),
            offer(
                "amzn1.adg.product.b",
                "Star Wars Outlaws",
                Some("B0DDMXR2WZ"),
            ),
        ]),
        at(1_000),
    )
    .await
    .expect("first refresh");

    luna.replace_catalog(
        &catalog(vec![
            offer("amzn1.adg.product.a", "Hogwarts Legacy", Some("B0CMQK7H1T")),
            offer("amzn1.luna.product.c", "Fortnite", None),
        ]),
        at(2_000),
    )
    .await
    .expect("second refresh");

    let titles: Vec<String> = luna
        .catalog()
        .await
        .expect("catalogue")
        .into_iter()
        .map(|offer| offer.title)
        .collect();
    assert_eq!(titles, ["Fortnite", "Hogwarts Legacy"]);

    let settings = luna.settings().await.expect("settings").expect("on");
    assert_eq!(settings.territory.as_deref(), Some("ES"));
    assert_eq!(settings.refreshed_at, Some(2_000));
}

#[tokio::test]
async fn a_change_of_country_keeps_the_catalogue() {
    let db = Database::in_memory().await.expect("database");
    let luna = LunaRepository(&db);
    luna.set_country("ES", "luna.amazon.es")
        .await
        .expect("switch on");
    luna.replace_catalog(
        &catalog(vec![offer("amzn1.adg.product.a", "Hogwarts Legacy", None)]),
        at(1_000),
    )
    .await
    .expect("refresh");

    // Amazon selects the catalogue from the connection, not from the country.
    luna.set_country("DE", "luna.amazon.de")
        .await
        .expect("change the country");

    assert_eq!(luna.catalog().await.expect("catalogue").len(), 1);
    let settings = luna.settings().await.expect("settings").expect("on");
    assert_eq!(settings.country, "DE");
    assert_eq!(settings.site, "luna.amazon.de");
    assert_eq!(settings.refreshed_at, Some(1_000));
}

#[tokio::test]
async fn switching_luna_off_touches_no_record_and_no_state() {
    let db = Database::in_memory().await.expect("database");
    let game = Game {
        id: GameId::new(),
        canonical_title: "Hogwarts Legacy".to_owned(),
        sort_title: "hogwarts legacy".to_owned(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    GameRepository(&db).upsert(&game).await.expect("record");
    let state = UserState {
        game_id: game.id,
        status: Some(PlayStatus::Backlog),
        rating: None,
        notes: Some("Wait for Luna".to_owned()),
        started_at: None,
        finished_at: None,
    };
    UserStateRepository(&db).save(&state).await.expect("state");

    let luna = LunaRepository(&db);
    luna.set_country("ES", "luna.amazon.es")
        .await
        .expect("switch on");
    luna.replace_catalog(
        &catalog(vec![offer("amzn1.adg.product.a", "Hogwarts Legacy", None)]),
        at(1_000),
    )
    .await
    .expect("refresh");
    luna.clear().await.expect("switch off");

    assert_eq!(luna.settings().await.expect("settings"), None);
    assert!(luna.catalog().await.expect("catalogue").is_empty());
    assert_eq!(GameRepository(&db).all().await.expect("records").len(), 1);
    assert_eq!(
        UserStateRepository(&db)
            .find(game.id)
            .await
            .expect("state")
            .and_then(|state| state.notes),
        Some("Wait for Luna".to_owned())
    );
}

#[tokio::test]
async fn the_migration_reverts() {
    let db = Database::in_memory().await.expect("database");
    LunaRepository(&db)
        .set_country("ES", "luna.amazon.es")
        .await
        .expect("switch on");

    db.undo_all().await.expect("revert the migrations");

    assert!(LunaRepository(&db).settings().await.is_err());
}

fn record(title: &str) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    }
}

/// A record with one wished copy whose store title is `store_title`.
async fn wished(db: &Database, title: &str, store_title: &str) -> GameId {
    let account = StoreAccountRepository(db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store: StoreId::Steam,
            account_ref: format!("account-{title}"),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("account");
    let game = record(title);
    GameRepository(db).upsert(&game).await.expect("record");
    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id: account,
        store: StoreId::Steam,
        store_app_id: title.to_owned(),
        kind: EntryKind::Wishlist,
        title: store_title.to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    };
    StoreEntryRepository(db)
        .upsert_many(std::slice::from_ref(&entry))
        .await
        .expect("entry");
    GameLinkRepository(db)
        .set_manual(&GameLink {
            game_id: game.id,
            store_entry_id: entry.id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await
        .expect("link");
    game.id
}

async fn spain_with(db: &Database, offers: Vec<LunaOffer>) {
    let luna = LunaRepository(db);
    luna.set_country("ES", "luna.amazon.es")
        .await
        .expect("switch on");
    luna.replace_catalog(&catalog(offers), at(1_000))
        .await
        .expect("refresh");
}

#[tokio::test]
async fn the_library_marks_a_record_by_its_title_or_the_title_of_a_copy() {
    let db = Database::in_memory().await.expect("database");
    let by_record = wished(&db, "Hogwarts Legacy", "Hogwarts Legacy Deluxe").await;
    let by_copy = wished(
        &db,
        "Guardians of the Galaxy",
        "Marvel's Guardians of the Galaxy",
    )
    .await;
    let not_there = wished(&db, "Disco Elysium", "Disco Elysium").await;
    spain_with(
        &db,
        vec![
            offer("amzn1.adg.product.a", "Hogwarts Legacy", Some("B0CMQK7H1T")),
            offer(
                "amzn1.adg.product.b",
                "Marvel's Guardians of the Galaxy",
                Some("B09DYWMB4F"),
            ),
        ],
    )
    .await;

    let rows = LibraryRepository(&db).all().await.expect("library");
    let mark = |id: GameId| {
        rows.iter()
            .find(|row| row.game_id == id)
            .expect("row")
            .luna
            .clone()
    };

    assert_eq!(
        mark(by_record).map(|m| m.url),
        Some("https://luna.amazon.es/game/hogwarts-legacy/B0CMQK7H1T".to_owned())
    );
    assert_eq!(
        mark(by_copy).map(|m| m.title),
        Some("Marvel's Guardians of the Galaxy".to_owned())
    );
    assert_eq!(mark(not_there), None);
}

#[tokio::test]
async fn a_record_added_after_the_refresh_is_marked_with_no_new_refresh() {
    let db = Database::in_memory().await.expect("database");
    spain_with(&db, vec![offer("amzn1.luna.product.c", "Fortnite", None)]).await;

    let game = record("Fortnite");
    GameRepository(&db).upsert(&game).await.expect("record");

    let row = LibraryRepository(&db)
        .one(game.id)
        .await
        .expect("row")
        .expect("visible");
    assert_eq!(
        row.luna.map(|m| m.url),
        Some("https://luna.amazon.es/game/fortnite".to_owned())
    );
}

#[tokio::test]
async fn with_luna_off_no_row_is_marked_and_the_json_has_no_luna_field() {
    let db = Database::in_memory().await.expect("database");
    let id = wished(&db, "Hogwarts Legacy", "Hogwarts Legacy").await;
    spain_with(
        &db,
        vec![offer("amzn1.adg.product.a", "Hogwarts Legacy", None)],
    )
    .await;
    LunaRepository(&db).clear().await.expect("switch off");

    let row = LibraryRepository(&db)
        .one(id)
        .await
        .expect("row")
        .expect("visible");
    assert_eq!(row.luna, None);

    let json = serde_json::to_value(&row).expect("json");
    assert!(json.get("luna").is_none());
    assert!(json.get("store_titles").is_none());
}
