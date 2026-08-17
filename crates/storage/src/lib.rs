//! Persistence in SQLite. All of the SQL of the project lives in this crate: if
//! a query appears in a different place, the boundary has broken.

mod mapping;
pub mod repositories;

use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("the migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("unexpected value in the column {column}: {value}")]
    Corrupt { column: &'static str, value: String },
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Opens — or creates — the database of the user and migrates it.
    ///
    /// Three settings that only the file database gets:
    ///
    /// - `journal_mode=WAL`: a write no longer copies the pages that it changes
    ///   into a journal beside the file and waits for the disc two times.
    /// - `synchronous=NORMAL`: with WAL this is the value that SQLite itself
    ///   recommends. The application loses at most the last transactions if the
    ///   machine loses power, and it never gets a corrupt file.
    /// - `busy_timeout`: a lock that is held waits five seconds instead of
    ///   giving an error at once.
    ///
    /// `in_memory()` does not get them: WAL needs a file, and there the journal
    /// means nothing.
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        Self::connect(options).await
    }

    /// A temporary database for the tests. The same migration path as the real
    /// database.
    pub async fn in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("the in-memory URL is a constant")
            .foreign_keys(true);
        Self::connect(options).await
    }

    async fn connect(options: SqliteConnectOptions) -> Result<Self> {
        // Only one connection: this is a desktop application for one user, and
        // thus the in-memory database of the tests does not disappear between
        // queries.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    pub async fn migrate(&self) -> Result<()> {
        MIGRATOR.run(&self.pool).await?;
        Ok(())
    }

    /// Reverts all of the migrations. It exists so that the schema has a
    /// reverse path that you can show, not because the application uses it in
    /// usual operation.
    pub async fn undo_all(&self) -> Result<()> {
        MIGRATOR.undo(&self.pool, 0).await?;
        Ok(())
    }

    /// The journal that this database uses: `wal` for a file, `memory` for the
    /// database of the tests.
    ///
    /// It exists for the tests. The SQL of the project lives in this crate, thus
    /// a test that wants to know cannot ask SQLite by itself.
    pub async fn journal_mode(&self) -> Result<String> {
        let row = sqlx::query("PRAGMA journal_mode")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get(0))
    }

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
