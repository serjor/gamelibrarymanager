//! Persistence in SQLite. All of the SQL of the project lives in this crate: if
//! a query appears in a different place, the boundary has broken.

mod mapping;
pub mod repositories;

use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

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
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
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

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
