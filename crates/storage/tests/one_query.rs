//! All of the library comes in **one** query, with one thousand games or with
//! one.
//!
//! Before, a clock examined this: "one thousand games in less than 500 ms". That
//! was incorrect in the two directions. It failed one time in six with the
//! machine busy with a compile, because clock time measures the load of the
//! computer as much as the code; and it still would not have found what it said
//! it watched, because one thousand queries to a local SQLite fit easily in one
//! half second.
//!
//! Now the test counts what is really important. sqlx records each statement
//! that it runs in the `sqlx::query` target, thus it is sufficient to install a
//! logger that counts them: one query for each game appears as 1000 and not as
//! "it is slow today".
//!
//! This test is alone in its file deliberately, and it must stay alone. The
//! SQLite driver runs the statements in a thread of its own, thus the counter
//! must be global; and a global counter is reliable only if no other test sends
//! queries at the same time. Each test file is a separate binary, but inside one
//! file the tests run in parallel: to add a second test here that touches the
//! database would make this one fail sometimes, which is exactly what this file
//! moved away from.

use std::sync::atomic::{AtomicUsize, Ordering};

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, StoreAccount, StoreAccountId, StoreEntry,
    StoreEntryId, StoreId,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, StoreAccountRepository,
    StoreEntryRepository,
};
use time::OffsetDateTime;

static QUERIES: AtomicUsize = AtomicUsize::new(0);

struct QueryCounter;

impl log::Log for QueryCounter {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target() == "sqlx::query"
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            QUERIES.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&self) {}
}

static COUNTER: QueryCounter = QueryCounter;

/// Sets the counter to zero and starts to count from here.
fn start_counting() {
    // If it was already installed it does not matter: what is important is the
    // zero after it.
    let _ = log::set_logger(&COUNTER);
    // sqlx records nothing if the level is not enabled.
    log::set_max_level(log::LevelFilter::Debug);
    QUERIES.store(0, Ordering::Relaxed);
}

fn queries_made() -> usize {
    QUERIES.load(Ordering::Relaxed)
}

fn game(title: &str) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: vec!["Shooter".to_owned()],
    }
}

#[tokio::test]
async fn one_thousand_games_come_in_one_query() {
    let db = Database::in_memory().await.expect("database");
    let account = StoreAccountRepository(&db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store: StoreId::Steam,
            account_ref: "account".to_owned(),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("account");

    let mut entries = Vec::with_capacity(1000);
    let mut links = Vec::with_capacity(1000);
    for i in 0..1000 {
        let record = game(&format!("Game {i:04}"));
        GameRepository(&db).upsert(&record).await.expect("record");
        let entry = StoreEntry {
            id: StoreEntryId::new(),
            account_id: account,
            store: StoreId::Steam,
            store_app_id: i.to_string(),
            kind: EntryKind::Owned,
            title: format!("Game {i:04}"),
            playtime_minutes: Some(i),
            acquired_at: None,
            cover_url: None,
            store_url: None,
            raw: serde_json::json!({}),
        };
        links.push(GameLink {
            game_id: record.id,
            store_entry_id: entry.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        });
        entries.push(entry);
    }
    StoreEntryRepository(&db)
        .upsert_many(&entries)
        .await
        .expect("entries");
    GameLinkRepository(&db)
        .rebuild_auto(&links)
        .await
        .expect("links");

    // The count starts here.
    start_counting();
    let rows = LibraryRepository(&db).all().await.expect("library");
    let made = queries_made();

    assert_eq!(rows.len(), 1000);
    assert!(rows[0].sort_title < rows[999].sort_title, "they are sorted");
    // An exact `1` is also a test of the counter itself: if the logger did not
    // install, this would be 0 and the test would still fail.
    assert_eq!(
        made, 1,
        "all of the library must come in one query; {made} statements for one \
         thousand games means that somebody added one query for each game"
    );
}
