//! The user can wish for a game on a device that no store reports.

use domain::{
    EntryKind, Game, GameId, PlatformFamily, PlayStatus, StoreAccount, StoreAccountId, StoreEntry,
    StoreEntryId, StoreId, UserState, matching,
};
use gamelibrarymanager_lib::testing::{
    Silent, WishTarget, add_manual_wish_for, remove_manual_wish_for, resolve_local,
    update_manual_wish_for,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, ManualWishRepository, PriceRepository,
    StoreAccountRepository, StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;

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

async fn add_store_copy(db: &Database, title: &str, app_id: &str) -> StoreEntryId {
    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: "manual-wish-test-account".to_owned(),
        display_name: None,
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let account_id = StoreAccountRepository(db)
        .upsert(&account)
        .await
        .expect("add the account");
    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store: StoreId::Steam,
        store_app_id: app_id.to_owned(),
        kind: EntryKind::Owned,
        title: title.to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::Value::Null,
    };
    StoreEntryRepository(db)
        .upsert_many(std::slice::from_ref(&entry))
        .await
        .expect("add the store copy");
    entry.id
}

#[tokio::test]
async fn an_existing_igdb_record_can_receive_a_manual_wish() {
    let db = Database::in_memory().await.expect("open the database");
    let mut record = game("The Witcher 3: Wild Hunt");
    record.igdb_id = Some(1942);
    record.cover_url = Some("https://example.test/witcher.jpg".to_owned());
    record.summary = Some("A role playing game".to_owned());
    GameRepository(&db)
        .upsert(&record)
        .await
        .expect("add the IGDB record");

    let row = add_manual_wish_for(
        &db,
        WishTarget::Existing(record.id),
        PlatformFamily::Playstation,
        "PS5",
    )
    .await
    .expect("wish for the existing record");

    assert_eq!(row.game_id, record.id);
    assert_eq!(row.title, record.canonical_title);
    assert_eq!(row.cover_url, record.cover_url);
    assert_eq!(row.summary, record.summary);
    assert!(row.owned_stores.is_empty());
    assert!(row.wishlist_stores.is_empty());
    assert_eq!(row.manual_wishes.len(), 1);
    assert_eq!(row.manual_wishes[0].game_id, record.id);
    assert_eq!(row.manual_wishes[0].family, PlatformFamily::Playstation);
    assert_eq!(row.manual_wishes[0].model, "PS5");
    assert_eq!(GameRepository(&db).all().await.expect("games").len(), 1);
}

#[tokio::test]
async fn a_free_title_can_have_two_devices_in_one_library_row() {
    let db = Database::in_memory().await.expect("open the database");
    let new_record = game("Beyond the Obsidian Veil");

    let first = add_manual_wish_for(
        &db,
        WishTarget::New(new_record.clone()),
        PlatformFamily::Pc,
        "PC",
    )
    .await
    .expect("add the free title for PC");
    let second = add_manual_wish_for(
        &db,
        WishTarget::Existing(new_record.id),
        PlatformFamily::Nintendo,
        "Switch",
    )
    .await
    .expect("add the same title for Switch");

    assert_eq!(first.game_id, new_record.id);
    assert_eq!(second.game_id, new_record.id);
    assert_eq!(second.title, "Beyond the Obsidian Veil");
    assert!(second.owned_stores.is_empty());
    assert!(second.wishlist_stores.is_empty());
    assert_eq!(second.manual_wishes.len(), 2);
    assert_eq!(
        second
            .manual_wishes
            .iter()
            .map(|wish| (wish.family, wish.model.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (PlatformFamily::Nintendo, "Switch"),
            (PlatformFamily::Pc, "PC")
        ]
    );

    // The public row shape keeps user wishes separate from store wishlists and
    // carries the wish identifier and device fields for the interface.
    let value = serde_json::to_value(&second).expect("serialize the library row");
    assert!(value["wishlist_stores"].as_array().unwrap().is_empty());
    assert_eq!(value["manual_wishes"].as_array().unwrap().len(), 2);
    assert_eq!(
        value["manual_wishes"][0]["game_id"],
        new_record.id.as_uuid().to_string()
    );
    assert_eq!(value["manual_wishes"][0]["family"], "nintendo");
    assert_eq!(value["manual_wishes"][0]["model"], "Switch");
    assert_eq!(value["manual_wishes"][1]["family"], "pc");
    assert_eq!(value["manual_wishes"][1]["model"], "PC");

    let listed = LibraryRepository(&db).all().await.expect("the library");
    assert_eq!(listed, vec![second]);
}

#[tokio::test]
async fn an_update_conflict_and_removal_keep_the_other_wish_and_user_state() {
    let db = Database::in_memory().await.expect("open the database");
    let record = game("Tunic");
    let first = add_manual_wish_for(
        &db,
        WishTarget::New(record.clone()),
        PlatformFamily::Playstation,
        "PS4",
    )
    .await
    .expect("add the PS4 wish");
    let second = add_manual_wish_for(
        &db,
        WishTarget::Existing(record.id),
        PlatformFamily::Playstation,
        "PS5",
    )
    .await
    .expect("add the PS5 wish");
    let first_id = first.manual_wishes[0].id;
    let second_id = second
        .manual_wishes
        .iter()
        .find(|wish| wish.model == "PS5")
        .expect("the PS5 wish")
        .id;

    UserStateRepository(&db)
        .save(&UserState {
            game_id: record.id,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("keep this note".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("save the user state");

    let conflict = update_manual_wish_for(&db, first_id, PlatformFamily::Playstation, " ps5 ")
        .await
        .expect_err("an existing device cannot be duplicated");
    assert!(conflict.to_string().contains("already in the wishlist"));
    assert_eq!(
        ManualWishRepository(&db)
            .for_game(record.id)
            .await
            .expect("the wishes after conflict")
            .iter()
            .map(|wish| wish.model.as_str())
            .collect::<Vec<_>>(),
        vec!["PS4", "PS5"]
    );

    let updated = update_manual_wish_for(&db, first_id, PlatformFamily::Xbox, " Series X ")
        .await
        .expect("change the first device");
    assert!(updated.manual_wishes.iter().any(|wish| {
        wish.id == first_id && wish.family == PlatformFamily::Xbox && wish.model == "Series X"
    }));

    let after_remove = remove_manual_wish_for(&db, first_id)
        .await
        .expect("remove the Xbox wish");
    assert_eq!(after_remove.manual_wishes.len(), 1);
    assert_eq!(after_remove.manual_wishes[0].id, second_id);
    assert_eq!(after_remove.manual_wishes[0].model, "PS5");
    assert_eq!(after_remove.status, Some(PlayStatus::Playing));
    assert_eq!(after_remove.rating, Some(9));
    assert_eq!(after_remove.notes.as_deref(), Some("keep this note"));
    assert!(
        GameRepository(&db)
            .find(record.id)
            .await
            .expect("read the record")
            .is_some()
    );
}

#[tokio::test]
async fn title_matching_and_orphan_cleanup_preserve_manual_records() {
    let db = Database::in_memory().await.expect("open the database");
    let manual_record = game("Celeste");
    let wish = add_manual_wish_for(
        &db,
        WishTarget::New(manual_record.clone()),
        PlatformFamily::Nintendo,
        "Switch",
    )
    .await
    .expect("add the manual title");
    let store_entry_id = add_store_copy(&db, "Celeste", "504230").await;

    resolve_local(&db, &Silent)
        .await
        .expect("match store copies by title");

    let store_game_id = GameLinkRepository(&db).all().await;
    let store_game_id = store_game_id
        .expect("the store link")
        .into_iter()
        .find(|link| link.store_entry_id == store_entry_id)
        .expect("the store copy has a game")
        .game_id;

    let rows = LibraryRepository(&db).all().await.expect("the library");
    assert_eq!(rows.len(), 2, "the manual and store records stay separate");
    let manual_row = rows
        .iter()
        .find(|row| row.game_id == manual_record.id)
        .expect("the manually wished record");
    let store_row = rows
        .iter()
        .find(|row| row.game_id == store_game_id)
        .expect("the store record");
    assert!(manual_row.owned_stores.is_empty());
    assert_eq!(manual_row.manual_wishes[0].id, wish.manual_wishes[0].id);
    assert_eq!(store_row.owned_stores, vec!["steam"]);
    assert!(store_row.manual_wishes.is_empty());

    assert_eq!(
        GameRepository(&db)
            .soft_delete_orphans()
            .await
            .expect("clean up orphan records"),
        0,
        "the live wish keeps its manual-only record"
    );
    remove_manual_wish_for(&db, wish.manual_wishes[0].id)
        .await
        .expect("remove the last wish");
    assert_eq!(
        GameRepository(&db)
            .soft_delete_orphans()
            .await
            .expect("preserve the record after the wish is removed"),
        0
    );
    assert!(
        GameRepository(&db)
            .find(manual_record.id)
            .await
            .expect("read the removed manual record")
            .is_some()
    );
    let rows = LibraryRepository(&db).all().await.expect("the library");
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| row.game_id == manual_record.id && row.manual_wishes.is_empty())
    );
    assert!(
        rows.iter()
            .any(|row| row.game_id == store_game_id && row.owned_stores == vec!["steam"])
    );
}

#[tokio::test]
async fn only_pc_manual_wishes_are_price_targets() {
    let db = Database::in_memory().await.expect("open the database");
    let pc_and_console = game("A Short Hike");
    add_manual_wish_for(
        &db,
        WishTarget::New(pc_and_console.clone()),
        PlatformFamily::Pc,
        "PC",
    )
    .await
    .expect("add a PC wish");
    add_manual_wish_for(
        &db,
        WishTarget::Existing(pc_and_console.id),
        PlatformFamily::Nintendo,
        "Switch",
    )
    .await
    .expect("add a Switch wish");

    let console_only = game("Spiritfarer");
    add_manual_wish_for(
        &db,
        WishTarget::New(console_only),
        PlatformFamily::Playstation,
        "PS5",
    )
    .await
    .expect("add a console-only wish");

    let targets = PriceRepository(&db).targets().await.expect("price targets");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].game_id, pc_and_console.id);
    assert_eq!(targets[0].title, "A Short Hike");
}
