//! The list of the Steam family is a cache that each synchronisation replaces
//! complete. What these tests watch is that a game that left the family goes,
//! that the IGDB record of a game that stays is kept, that nothing of the
//! family reaches a record or what the user wrote, and that the library marks a
//! record by its appid, by its IGDB record or by its title when it reads.

use std::collections::HashMap;

use domain::{
    EntryKind, FamilyApp, FamilyGroup, FamilyOwner, Game, GameId, GameLink, LinkMethod, PlayStatus,
    StoreAccount, StoreAccountId, StoreEntry, StoreEntryId, StoreId, UserState,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, SteamFamilyRepository,
    SteamFamilySettings, StoreAccountRepository, StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;

const GROUP: &str = "1234567890123456789";

fn member() -> FamilyGroup {
    FamilyGroup::Member(GROUP.to_owned())
}

fn app(app_id: &str, title: &str, owner: &str) -> FamilyApp {
    FamilyApp {
        app_id: app_id.to_owned(),
        title: title.to_owned(),
        owners: vec![FamilyOwner {
            steam_id: format!("7656119000000000{}", owner.len()),
            name: Some(owner.to_owned()),
        }],
        igdb_id: None,
    }
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).expect("a valid moment")
}

#[tokio::test]
async fn with_no_row_the_family_is_switched_off() {
    let db = Database::in_memory().await.expect("database");
    let family = SteamFamilyRepository(&db);

    assert_eq!(family.settings().await.expect("settings"), None);

    family.enable().await.expect("switch on");
    assert_eq!(
        family.settings().await.expect("settings"),
        Some(SteamFamilySettings {
            family_groupid: None,
            refreshed_at: None,
            last_error: None,
            games: 0,
        })
    );
}

#[tokio::test]
async fn a_synchronisation_replaces_the_list_and_keeps_the_igdb_record_of_what_stays() {
    let db = Database::in_memory().await.expect("database");
    let family = SteamFamilyRepository(&db);
    family.enable().await.expect("switch on");

    family
        .replace(
            &member(),
            &[
                app("1145360", "Hades", "Ana"),
                app("632470", "Disco Elysium", "Luis"),
            ],
            at(1_000),
        )
        .await
        .expect("first list");
    assert_eq!(
        family.missing_igdb().await.expect("missing"),
        ["1145360", "632470"]
    );
    family
        .set_igdb_ids(&HashMap::from([("1145360".to_owned(), 113_112)]))
        .await
        .expect("the identity pass");
    family
        .record_error("the sign in was closed")
        .await
        .expect("error");

    // Luis left the family, and a new game arrived.
    family
        .replace(
            &member(),
            &[
                app("1145360", "Hades", "Ana"),
                app("105600", "Terraria", "Ana"),
            ],
            at(2_000),
        )
        .await
        .expect("second list");

    let apps = family.apps().await.expect("list");
    let summary: Vec<(&str, Option<i64>)> = apps
        .iter()
        .map(|app| (app.title.as_str(), app.igdb_id))
        .collect();
    assert_eq!(summary, [("Hades", Some(113_112)), ("Terraria", None)]);

    let settings = family.settings().await.expect("settings").expect("on");
    assert_eq!(settings.family_groupid.as_deref(), Some(GROUP));
    assert_eq!(settings.refreshed_at, Some(2_000));
    assert_eq!(settings.last_error, None, "a stage that ended clears it");
    assert_eq!(settings.games, 2);
}

#[tokio::test]
async fn an_error_keeps_the_list_of_the_last_stage() {
    let db = Database::in_memory().await.expect("database");
    let family = SteamFamilyRepository(&db);
    family.enable().await.expect("switch on");
    family
        .replace(&member(), &[app("1145360", "Hades", "Ana")], at(1_000))
        .await
        .expect("list");

    family
        .record_error("Steam did not accept the session")
        .await
        .expect("error");

    let settings = family.settings().await.expect("settings").expect("on");
    assert_eq!(
        settings.last_error.as_deref(),
        Some("Steam did not accept the session")
    );
    assert_eq!(settings.games, 1);
    assert_eq!(settings.refreshed_at, Some(1_000));
}

#[tokio::test]
async fn a_user_with_no_family_has_an_empty_list() {
    let db = Database::in_memory().await.expect("database");
    let family = SteamFamilyRepository(&db);
    family.enable().await.expect("switch on");
    family
        .replace(&member(), &[app("1145360", "Hades", "Ana")], at(1_000))
        .await
        .expect("list");

    family
        .replace(&FamilyGroup::NotMember, &[], at(2_000))
        .await
        .expect("no family");

    let settings = family.settings().await.expect("settings").expect("on");
    assert_eq!(settings.family_groupid, None);
    assert_eq!(settings.games, 0);
}

#[tokio::test]
async fn a_list_does_not_come_back_after_the_user_switched_the_family_off() {
    let db = Database::in_memory().await.expect("database");
    let family = SteamFamilyRepository(&db);

    let written = family
        .replace(&member(), &[app("1145360", "Hades", "Ana")], at(1_000))
        .await
        .expect("replace");

    assert!(!written);
    assert_eq!(family.settings().await.expect("settings"), None);
    assert!(family.apps().await.expect("list").is_empty());
}

#[tokio::test]
async fn switching_the_family_off_touches_no_record_and_no_state() {
    let db = Database::in_memory().await.expect("database");
    let game = record("Hades", None);
    GameRepository(&db).upsert(&game).await.expect("record");
    let state = UserState {
        game_id: game.id,
        status: Some(PlayStatus::Backlog),
        rating: None,
        notes: Some("Ask Ana".to_owned()),
        started_at: None,
        finished_at: None,
    };
    UserStateRepository(&db).save(&state).await.expect("state");

    let family = SteamFamilyRepository(&db);
    family.enable().await.expect("switch on");
    family
        .replace(&member(), &[app("1145360", "Hades", "Ana")], at(1_000))
        .await
        .expect("list");
    family.disable().await.expect("switch off");

    assert_eq!(family.settings().await.expect("settings"), None);
    assert!(family.apps().await.expect("list").is_empty());
    assert_eq!(GameRepository(&db).all().await.expect("records").len(), 1);
    assert_eq!(
        UserStateRepository(&db)
            .find(game.id)
            .await
            .expect("state")
            .and_then(|state| state.notes),
        Some("Ask Ana".to_owned())
    );
}

#[tokio::test]
async fn the_migration_reverts() {
    let db = Database::in_memory().await.expect("database");
    SteamFamilyRepository(&db)
        .enable()
        .await
        .expect("switch on");

    db.undo_all().await.expect("revert the migrations");

    assert!(SteamFamilyRepository(&db).settings().await.is_err());
}

fn record(title: &str, igdb_id: Option<i64>) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    }
}

/// A record with one wished copy of `store`.
async fn wished(
    db: &Database,
    store: StoreId,
    app_id: &str,
    title: &str,
    store_title: &str,
    igdb_id: Option<i64>,
) -> GameId {
    let account = StoreAccountRepository(db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store,
            account_ref: format!("account-{app_id}"),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("account");
    let game = record(title, igdb_id);
    GameRepository(db).upsert(&game).await.expect("record");
    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id: account,
        store,
        store_app_id: app_id.to_owned(),
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

async fn family_with(db: &Database, apps: &[FamilyApp]) {
    let family = SteamFamilyRepository(db);
    family.enable().await.expect("switch on");
    family
        .replace(&member(), apps, at(1_000))
        .await
        .expect("list");
}

#[tokio::test]
async fn the_library_marks_a_record_by_appid_by_igdb_and_by_title() {
    let db = Database::in_memory().await.expect("database");
    // A Steam wish whose record IGDB named in a different way.
    let by_app = wished(
        &db,
        StoreId::Steam,
        "1145360",
        "Hades: Battle Out of Hell",
        "Steam 1145360",
        None,
    )
    .await;
    // A GOG wish: another identifier and another title, the same IGDB record.
    let by_igdb = wished(
        &db,
        StoreId::Gog,
        "1207658924",
        "The Witcher 3",
        "The Witcher 3: Wild Hunt - Complete Edition",
        Some(1942),
    )
    .await;
    // A record with no copy and no IGDB, as a manual wish leaves it.
    let by_title = record("Disco Elysium", None);
    GameRepository(&db).upsert(&by_title).await.expect("record");
    let similar = record("Disco Elysium 2", None);
    GameRepository(&db).upsert(&similar).await.expect("record");

    let mut witcher = app("292030", "The Witcher 3: Wild Hunt", "Luis");
    witcher.igdb_id = Some(1942);
    family_with(
        &db,
        &[
            app("1145360", "Hades", "Ana"),
            witcher,
            app("632470", "Disco Elysium", "Ana"),
        ],
    )
    .await;

    let rows = LibraryRepository(&db).all().await.expect("library");
    let mark = |id: GameId| {
        rows.iter()
            .find(|row| row.game_id == id)
            .expect("row")
            .steam_family
            .clone()
    };

    assert_eq!(mark(by_app).map(|m| m.app_id), Some("1145360".to_owned()));
    let witcher = mark(by_igdb).expect("marked by IGDB");
    assert_eq!(witcher.app_id, "292030");
    assert_eq!(witcher.owners, ["Luis"]);
    assert_eq!(
        mark(by_title.id).map(|m| m.app_id),
        Some("632470".to_owned())
    );
    assert_eq!(mark(similar.id), None);
}

#[tokio::test]
async fn with_the_family_off_no_row_is_marked_and_the_json_has_no_family_field() {
    let db = Database::in_memory().await.expect("database");
    let id = wished(
        &db,
        StoreId::Steam,
        "1145360",
        "Hades",
        "Hades",
        Some(113_112),
    )
    .await;
    family_with(&db, &[app("1145360", "Hades", "Ana")]).await;
    SteamFamilyRepository(&db)
        .disable()
        .await
        .expect("switch off");

    let row = LibraryRepository(&db)
        .one(id)
        .await
        .expect("row")
        .expect("visible");
    assert_eq!(row.steam_family, None);

    let json = serde_json::to_value(&row).expect("json");
    assert!(json.get("steam_family").is_none());
    assert!(json.get("steam_app_ids").is_none());
    assert!(json.get("igdb_id").is_none());
}

#[tokio::test]
async fn a_record_added_after_the_synchronisation_is_marked_with_no_new_one() {
    let db = Database::in_memory().await.expect("database");
    family_with(&db, &[app("632470", "Disco Elysium", "Ana")]).await;

    let game = record("Disco Elysium", None);
    GameRepository(&db).upsert(&game).await.expect("record");

    let row = LibraryRepository(&db)
        .one(game.id)
        .await
        .expect("row")
        .expect("visible");
    assert_eq!(
        row.steam_family.map(|m| m.owners),
        Some(vec!["Ana".to_owned()])
    );
}
