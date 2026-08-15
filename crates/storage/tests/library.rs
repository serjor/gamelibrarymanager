//! The library query: that it collects correctly the data attached to a
//! record — stores, hours, status, genres — even if it comes from more than one
//! store.
//!
//! That it is **one** query is tested separately, in `one_query.rs`, because a
//! count of statements needs a binary of its own.
use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, PlayStatus, StoreAccount, StoreAccountId,
    StoreEntry, StoreEntryId, StoreId, UserState,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, StoreAccountRepository,
    StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;

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
        .expect("account")
}

fn game(title: &str, genres: &[&str]) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id: None,
        cover_url: Some("https://images.igdb.com/x.jpg".to_owned()),
        summary: None,
        released_at: OffsetDateTime::from_unix_timestamp(1_571_270_400).ok(),
        genres: genres.iter().map(|g| (*g).to_owned()).collect(),
    }
}

fn entry(
    account_id: StoreAccountId,
    store: StoreId,
    app_id: &str,
    kind: EntryKind,
    playtime: i64,
) -> StoreEntry {
    StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind,
        title: "does not matter".to_owned(),
        playtime_minutes: Some(playtime),
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    }
}

/// What the Steam connector really keeps, which is where the last time played
/// comes from: `steam/parse.rs` puts `rtime_last_played` in the raw JSON.
fn as_steam_keeps_it(mut entry: StoreEntry, last: i64) -> StoreEntry {
    entry.cover_url = Some("https://cdn.cloudflare.steamstatic.com/header.jpg".to_owned());
    entry.store_url = Some("https://store.steampowered.com/app/632470".to_owned());
    entry.raw = serde_json::json!({ "appid": 632_470, "rtime_last_played": last });
    entry
}

#[tokio::test]
async fn one_record_with_two_stores_adds_the_hours_and_lists_the_two() {
    let db = Database::in_memory().await.expect("database");
    let steam = account(&db, StoreId::Steam).await;
    let gog = account(&db, StoreId::Gog).await;

    let record = game("Disco Elysium", &["RPG", "Adventure"]);
    GameRepository(&db).upsert(&record).await.expect("record");

    let in_steam = as_steam_keeps_it(
        entry(steam, StoreId::Steam, "632470", EntryKind::Owned, 1200),
        1_700_000_000,
    );
    let mut in_gog = entry(gog, StoreId::Gog, "151239", EntryKind::Owned, 300);
    in_gog.cover_url = Some("https://images.gog-statics.com/logo.jpg".to_owned());
    in_gog.store_url = Some("https://www.gog.com/game/disco_elysium".to_owned());
    let wished = entry(gog, StoreId::Gog, "999", EntryKind::Wishlist, 0);
    StoreEntryRepository(&db)
        .upsert_many(&[in_steam.clone(), in_gog.clone(), wished.clone()])
        .await
        .expect("entries");

    GameLinkRepository(&db)
        .rebuild_auto(&[
            GameLink {
                game_id: record.id,
                store_entry_id: in_steam.id,
                confidence: 1.0,
                method: LinkMethod::Auto,
            },
            GameLink {
                game_id: record.id,
                store_entry_id: in_gog.id,
                confidence: 1.0,
                method: LinkMethod::Auto,
            },
            GameLink {
                game_id: record.id,
                store_entry_id: wished.id,
                confidence: 1.0,
                method: LinkMethod::Auto,
            },
        ])
        .await
        .expect("links");

    UserStateRepository(&db)
        .save(&UserState {
            game_id: record.id,
            status: Some(PlayStatus::Playing),
            rating: Some(10),
            notes: Some("a wonder".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("status");

    let rows = LibraryRepository(&db).all().await.expect("library");
    assert_eq!(rows.len(), 1, "two copies are one game");

    let row = &rows[0];
    assert_eq!(row.owned_stores, vec!["gog".to_owned(), "steam".to_owned()]);
    assert_eq!(row.wishlist_stores, vec!["gog".to_owned()]);
    assert_eq!(
        row.playtime_minutes, 1500,
        "the hours of the two stores are added"
    );
    assert_eq!(row.status, Some(PlayStatus::Playing));
    assert_eq!(row.rating, Some(10));
    assert_eq!(row.genres, vec!["RPG".to_owned(), "Adventure".to_owned()]);
    assert_eq!(row.release_year, Some(2019));

    // With a copy in the two stores, the image and the link come both from
    // Steam: its header is made to be seen wide and GOG gives only the logo.
    // That they come from the same copy is what prevents the image of one store
    // with the link of the other.
    assert_eq!(
        row.store_cover_url.as_deref(),
        Some("https://cdn.cloudflare.steamstatic.com/header.jpg")
    );
    assert_eq!(
        row.store_url.as_deref(),
        Some("https://store.steampowered.com/app/632470")
    );
    assert_eq!(
        row.last_played_at,
        Some(1_700_000_000),
        "the last time played comes from the raw JSON of Steam"
    );
}

#[tokio::test]
async fn with_no_steam_copy_there_is_no_last_time_played() {
    // GOG publishes neither hours nor the date of the last game, thus the column
    // stays empty even if the user has played the game a lot. It is a limit of
    // the store, not a defect: the interface must be able to tell it from a
    // "never played", and thus it is `None` and not a zero.
    let db = Database::in_memory().await.expect("database");
    let gog = account(&db, StoreId::Gog).await;

    let record = game("Cultist Simulator", &["Simulation"]);
    GameRepository(&db).upsert(&record).await.expect("record");

    let in_gog = entry(gog, StoreId::Gog, "1207660103", EntryKind::Owned, 90);
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&in_gog))
        .await
        .expect("entries");
    GameLinkRepository(&db)
        .rebuild_auto(&[GameLink {
            game_id: record.id,
            store_entry_id: in_gog.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        }])
        .await
        .expect("links");

    let rows = LibraryRepository(&db).all().await.expect("library");
    assert_eq!(rows[0].last_played_at, None);
    assert_eq!(rows[0].playtime_minutes, 90, "GOG does give the hours");
}

#[tokio::test]
async fn a_steam_game_never_started_does_not_count_as_played_in_1970() {
    // Steam sends `rtime_last_played: 0` for the games never opened. Without a
    // change to NULL, a sort by the last time played would put the games never
    // started as the oldest of the library and not apart.
    let db = Database::in_memory().await.expect("database");
    let steam = account(&db, StoreId::Steam).await;

    let record = game("Prey", &["Action"]);
    GameRepository(&db).upsert(&record).await.expect("record");

    let never_started = as_steam_keeps_it(
        entry(steam, StoreId::Steam, "480490", EntryKind::Owned, 0),
        0,
    );
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&never_started))
        .await
        .expect("entries");
    GameLinkRepository(&db)
        .rebuild_auto(&[GameLink {
            game_id: record.id,
            store_entry_id: never_started.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        }])
        .await
        .expect("links");

    let rows = LibraryRepository(&db).all().await.expect("library");
    assert_eq!(rows[0].last_played_at, None);
}
