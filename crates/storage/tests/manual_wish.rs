//! The wishes a user writes for a game and a device.

use domain::{Game, GameId, ManualWish, ManualWishId, PlatformFamily};
use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection, SqliteJournalMode};
use sqlx::{Row, query, raw_sql};
use std::path::{Path, PathBuf};
use storage::Database;
use storage::repositories::{GameRepository, ManualWishRepository};

async fn add_game(db: &Database) -> GameId {
    let game = Game {
        id: GameId::new(),
        canonical_title: "Disco Elysium".to_owned(),
        sort_title: "disco elysium".to_owned(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    GameRepository(db)
        .upsert(&game)
        .await
        .expect("add the game");
    game.id
}

fn wish(game_id: GameId, family: PlatformFamily, model: &str) -> ManualWish {
    ManualWish {
        id: ManualWishId::new(),
        game_id,
        family,
        model: model.to_owned(),
    }
}

fn assert_unique_violation<T: std::fmt::Debug>(result: storage::Result<T>) {
    assert!(
        matches!(result, Err(storage::StorageError::Database(ref error))
            if error.as_database_error().is_some_and(|database_error| database_error.is_unique_violation())),
        "expected a unique constraint error, got {result:?}"
    );
}

async fn inspect_file(path: &Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal),
    )
    .await
    .expect("open the temporary database for inspection")
}

fn backup_paths(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .expect("read the temporary directory")
        .map(|entry| entry.expect("read a directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("library.db.bak-8"))
        })
        .collect();
    paths.sort();
    paths
}

#[tokio::test]
async fn one_game_can_have_two_manual_devices() {
    let db = Database::in_memory().await.expect("open the database");
    let game_id = add_game(&db).await;
    let ps4 = wish(game_id, PlatformFamily::Playstation, "PS4");
    let ps5 = wish(game_id, PlatformFamily::Playstation, "PS5");

    let repo = ManualWishRepository(&db);
    repo.add(&ps4).await.expect("add the PS4 wish");
    repo.add(&ps5).await.expect("add the PS5 wish");

    assert_eq!(
        repo.for_game(game_id).await.expect("list wishes"),
        vec![ps4, ps5]
    );
}

#[tokio::test]
async fn duplicate_normalized_devices_are_rejected() {
    let db = Database::in_memory().await.expect("open the database");
    let game_id = add_game(&db).await;
    let original = wish(game_id, PlatformFamily::Playstation, "PS5");
    let duplicate = wish(game_id, PlatformFamily::Playstation, "  ps5  ");
    let repo = ManualWishRepository(&db);

    repo.add(&original).await.expect("add the first wish");
    assert_unique_violation(repo.add(&duplicate).await);

    assert_eq!(
        repo.for_game(game_id).await.expect("list wishes"),
        vec![original]
    );
}

#[tokio::test]
async fn editing_to_an_existing_device_returns_a_conflict_without_changing_the_row() {
    let db = Database::in_memory().await.expect("open the database");
    let game_id = add_game(&db).await;
    let ps4 = wish(game_id, PlatformFamily::Playstation, "PS4");
    let ps5 = wish(game_id, PlatformFamily::Playstation, "PS5");
    let repo = ManualWishRepository(&db);
    repo.add(&ps4).await.expect("add the PS4 wish");
    repo.add(&ps5).await.expect("add the PS5 wish");

    assert_unique_violation(
        repo.update_device(ps4.id, PlatformFamily::Playstation, "  ps5  ")
            .await,
    );

    assert_eq!(
        repo.find(ps4.id).await.expect("read the PS4 wish"),
        Some(ps4)
    );
    assert_eq!(
        repo.find(ps5.id).await.expect("read the PS5 wish"),
        Some(ps5)
    );
}

#[tokio::test]
async fn a_wish_needs_an_existing_game() {
    let db = Database::in_memory().await.expect("open the database");
    let missing_game = GameId::new();
    let error = ManualWishRepository(&db)
        .add(&wish(missing_game, PlatformFamily::Pc, "PC"))
        .await
        .expect_err("the foreign key must reject a missing game");

    assert!(
        error.to_string().contains("FOREIGN KEY constraint failed"),
        "expected a foreign key error, got {error:?}"
    );
}

#[tokio::test]
async fn wishes_persist_and_removal_keeps_a_row_that_can_be_recreated() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = dir.path().join("library.db");
    let db = Database::open(&path).await.expect("open the file database");
    let game_id = add_game(&db).await;
    let original = wish(game_id, PlatformFamily::Nintendo, "Switch");
    ManualWishRepository(&db)
        .add(&original)
        .await
        .expect("add the wish");
    drop(db);

    let db = Database::open(&path)
        .await
        .expect("reopen the file database");
    let repo = ManualWishRepository(&db);
    assert_eq!(
        repo.find(original.id).await.expect("read the wish"),
        Some(original.clone())
    );
    assert!(repo.remove(original.id).await.expect("remove the wish"));
    assert_eq!(
        repo.find(original.id).await.expect("read removed wish"),
        None
    );

    let recreated = wish(game_id, original.family, &original.model);
    repo.add(&recreated)
        .await
        .expect("the removed device can be wished for again");
    assert_eq!(
        repo.for_game(game_id).await.expect("list wishes"),
        vec![recreated]
    );
    drop(db);

    let mut connection = inspect_file(&path).await;
    let rows = query("SELECT COUNT(*) AS n FROM manual_wish")
        .fetch_one(&mut connection)
        .await
        .expect("count all wish rows");
    let deleted = query("SELECT COUNT(*) AS n FROM manual_wish WHERE deleted_at IS NOT NULL")
        .fetch_one(&mut connection)
        .await
        .expect("count removed wish rows");
    assert_eq!(rows.get::<i64, _>("n"), 2, "the removed row remains stored");
    assert_eq!(deleted.get::<i64, _>("n"), 1);
    connection.close().await.expect("close the inspector");
}

#[tokio::test]
async fn an_existing_database_is_backed_up_before_migration_0008() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = dir.path().join("library.db");
    let db = Database::open(&path)
        .await
        .expect("create the current schema");
    let game_id = add_game(&db).await;
    drop(db);

    // Make this temporary file represent schema 0007 so opening it applies
    // migration 0008 through the normal Database path.
    let mut connection = inspect_file(&path).await;
    raw_sql(include_str!(
        "../../../migrations/0008_manual_wish.down.sql"
    ))
    .execute(&mut connection)
    .await
    .expect("revert migration 0008 in the temporary fixture");
    query("DELETE FROM _sqlx_migrations WHERE version = 8")
        .execute(&mut connection)
        .await
        .expect("mark migration 0008 as pending in the fixture");
    connection
        .close()
        .await
        .expect("close the old-schema fixture");

    let migrated = Database::open(&path)
        .await
        .expect("back up and migrate the old-schema file");
    assert!(
        GameRepository(&migrated)
            .find(game_id)
            .await
            .expect("read the existing game")
            .is_some(),
        "the game survives the migration"
    );
    drop(migrated);

    let backups = backup_paths(dir.path());
    assert_eq!(backups.len(), 1, "the pending migration made one backup");
    let mut backup = inspect_file(&backups[0]).await;
    let saved_game = query("SELECT id FROM game WHERE id = ?")
        .bind(game_id.as_uuid().to_string())
        .fetch_optional(&mut backup)
        .await
        .expect("query the pre-migration copy");
    assert!(saved_game.is_some(), "the backup keeps the existing game");
    let manual_wish_table = query(
        "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name = 'manual_wish'",
    )
    .fetch_one(&mut backup)
    .await
    .expect("inspect the pre-migration schema");
    assert_eq!(
        manual_wish_table.get::<i64, _>("n"),
        0,
        "the backup is the schema from before migration 0008"
    );
    backup.close().await.expect("close the backup inspector");
}
