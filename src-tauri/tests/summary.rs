//! The summary costs four statements, and not the complete library.
//!
//! The four numbers of the header were the lengths of four lists: every row of
//! `store_entry` and of `game`, with every `raw` JSON parsed, to give four
//! numbers and throw the rest away. Now the database counts.
//!
//! It counts the statements with the same logger as
//! `crates/storage/tests/one_query.rs`, and for the same reason **it must be
//! alone in its file**: the SQLite driver runs the statements in a thread of its
//! own, thus the counter is global, and a global counter is reliable only if no
//! other test sends queries at the same time. Each test file is a separate
//! binary; the tests inside one file are not.

use std::sync::atomic::{AtomicUsize, Ordering};

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, StoreAccount, StoreAccountId, StoreEntry,
    StoreEntryId, StoreId,
};
use gamelibrarymanager_lib::testing::summary;
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, StoreAccountRepository, StoreEntryRepository,
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

fn start_counting() {
    let _ = log::set_logger(&COUNTER);
    // sqlx records nothing if the level is not enabled.
    log::set_max_level(log::LevelFilter::Debug);
    QUERIES.store(0, Ordering::Relaxed);
}

fn queries_made() -> usize {
    QUERIES.load(Ordering::Relaxed)
}

#[tokio::test]
async fn the_summary_costs_four_statements_and_not_the_library() {
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

    // Three hundred copies that you have, one hundred wished for, and a record
    // for each copy that you have except the last fifty: those fifty are the
    // review queue.
    let mut entries = Vec::new();
    let mut links = Vec::new();
    for i in 0..300 {
        let entry = StoreEntry {
            id: StoreEntryId::new(),
            account_id: account,
            store: StoreId::Steam,
            store_app_id: i.to_string(),
            kind: EntryKind::Owned,
            title: format!("Game {i:04}"),
            playtime_minutes: None,
            acquired_at: None,
            cover_url: None,
            store_url: None,
            raw: serde_json::json!({}),
        };
        if i < 250 {
            let record = Game {
                id: GameId::new(),
                canonical_title: format!("Game {i:04}"),
                sort_title: format!("game {i:04}"),
                igdb_id: None,
                cover_url: None,
                summary: None,
                released_at: None,
                genres: vec!["Shooter".to_owned()],
            };
            GameRepository(&db).upsert(&record).await.expect("record");
            links.push(GameLink {
                game_id: record.id,
                store_entry_id: entry.id,
                confidence: 1.0,
                method: LinkMethod::Auto,
            });
        }
        entries.push(entry);
    }
    for i in 0..100 {
        entries.push(StoreEntry {
            id: StoreEntryId::new(),
            account_id: account,
            store: StoreId::Steam,
            store_app_id: format!("w{i}"),
            kind: EntryKind::Wishlist,
            title: format!("Wished {i:04}"),
            playtime_minutes: None,
            acquired_at: None,
            cover_url: None,
            store_url: None,
            raw: serde_json::json!({}),
        });
    }
    StoreEntryRepository(&db)
        .upsert_many(&entries)
        .await
        .expect("copies");
    GameLinkRepository(&db)
        .rebuild_auto(&links)
        .await
        .expect("links");

    // The count starts here.
    start_counting();
    let summary = summary(&db).await.expect("the summary");
    let made = queries_made();

    assert_eq!(summary.owned, 300);
    assert_eq!(summary.wishlist, 100);
    assert_eq!(summary.games, 250);
    // The fifty copies with no record, and the hundred wished for, which have
    // no record either.
    assert_eq!(summary.pending_review, 150);

    // An exact `4` is also a test of the counter itself: if the logger did not
    // install, this would be 0 and the test would still fail.
    assert_eq!(
        made, 4,
        "the summary is four counts; {made} statements means that somebody \
         asked for a list again to read its length"
    );
}
