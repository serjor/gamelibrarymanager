//! The logical delete of the copies that a store no longer shows, with a
//! library that is large.
//!
//! Before, the comparison was a `NOT IN` with one placeholder for each
//! identifier that the store gave. Two thousand copies made a statement with two
//! thousand parameters, and the answer that looks correct — to divide it into
//! batches — deletes everything that is in the other batch. The identifiers now
//! go into a temporary table, and the comparison is made one time against that
//! table.

use domain::{EntryKind, StoreAccount, StoreAccountId, StoreEntry, StoreEntryId, StoreId};
use storage::Database;
use storage::repositories::{StoreAccountRepository, StoreEntryRepository};
use time::OffsetDateTime;

/// More than the 500 of one batch, and more than the parameters that the old
/// statement could carry.
const TOTAL: usize = 2_000;
/// What the store still shows on the second synchronisation.
const STILL_THERE: usize = 1_500;

fn entry(account_id: StoreAccountId, app_id: usize) -> StoreEntry {
    StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store: StoreId::Steam,
        store_app_id: app_id.to_string(),
        kind: EntryKind::Owned,
        title: format!("Game {app_id:04}"),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    }
}

#[tokio::test]
async fn a_library_of_two_thousand_copies_deletes_the_five_hundred_that_left() {
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

    let entries: Vec<StoreEntry> = (0..TOTAL).map(|i| entry(account, i)).collect();
    StoreEntryRepository(&db)
        .upsert_many(&entries)
        .await
        .expect("the first synchronisation");

    // The store now shows only the first 1,500.
    let seen: Vec<String> = (0..STILL_THERE).map(|i| i.to_string()).collect();
    let deleted = StoreEntryRepository(&db)
        .soft_delete_missing(account, EntryKind::Owned, &seen)
        .await
        .expect("the logical delete");

    assert_eq!(
        deleted,
        (TOTAL - STILL_THERE) as u64,
        "only the copies that the store no longer shows get the logical delete"
    );

    let active = StoreEntryRepository(&db)
        .active(EntryKind::Owned)
        .await
        .expect("the copies that stay");
    assert_eq!(active.len(), STILL_THERE);
    assert_eq!(
        StoreEntryRepository(&db)
            .count_active(EntryKind::Owned)
            .await
            .expect("the count"),
        STILL_THERE as i64,
        "the count says the same as the list"
    );

    // And the identifiers that stay are exactly the ones that the store showed.
    let mut app_ids: Vec<i64> = active
        .iter()
        .map(|e| e.store_app_id.parse().expect("a number"))
        .collect();
    app_ids.sort_unstable();
    assert_eq!(app_ids.first(), Some(&0));
    assert_eq!(app_ids.last(), Some(&((STILL_THERE - 1) as i64)));
}

/// A second pass with the same list deletes nothing more, and the temporary
/// table of the pass before leaves nothing behind it.
#[tokio::test]
async fn the_same_list_two_times_deletes_one_time() {
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

    let entries: Vec<StoreEntry> = (0..600).map(|i| entry(account, i)).collect();
    StoreEntryRepository(&db)
        .upsert_many(&entries)
        .await
        .expect("copies");

    let seen: Vec<String> = (0..500).map(|i| i.to_string()).collect();
    let first = StoreEntryRepository(&db)
        .soft_delete_missing(account, EntryKind::Owned, &seen)
        .await
        .expect("the first pass");
    let second = StoreEntryRepository(&db)
        .soft_delete_missing(account, EntryKind::Owned, &seen)
        .await
        .expect("the second pass");

    assert_eq!(first, 100);
    assert_eq!(second, 0, "there is nothing more to delete");
    assert_eq!(
        StoreEntryRepository(&db)
            .count_active(EntryKind::Owned)
            .await
            .expect("the count"),
        500
    );
}

/// A store that gives no identifier deletes all of the copies of that account,
/// which is what an empty library means. The temporary table is empty, and an
/// empty `NOT IN (SELECT …)` is true for every row.
#[tokio::test]
async fn a_store_with_nothing_deletes_all_of_the_copies_of_that_account() {
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

    let entries: Vec<StoreEntry> = (0..10).map(|i| entry(account, i)).collect();
    StoreEntryRepository(&db)
        .upsert_many(&entries)
        .await
        .expect("copies");

    let deleted = StoreEntryRepository(&db)
        .soft_delete_missing(account, EntryKind::Owned, &[])
        .await
        .expect("the logical delete");

    assert_eq!(deleted, 10);
    assert_eq!(
        StoreEntryRepository(&db)
            .count_active(EntryKind::Owned)
            .await
            .expect("the count"),
        0
    );
}

/// The file database writes with WAL; the database of the tests does not need
/// it.
#[tokio::test]
async fn a_file_database_uses_wal_and_the_one_in_memory_does_not() {
    let dir = tempfile::tempdir().expect("temporal");
    let file = Database::open(&dir.path().join("library.db"))
        .await
        .expect("open the file database");

    assert_eq!(
        file.journal_mode()
            .await
            .expect("the journal")
            .to_lowercase(),
        "wal"
    );

    let memory = Database::in_memory().await.expect("database");
    assert_ne!(
        memory
            .journal_mode()
            .await
            .expect("the journal")
            .to_lowercase(),
        "wal",
        "WAL needs a file: `in_memory()` is not touched"
    );
}
