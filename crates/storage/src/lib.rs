//! Persistence in SQLite. All of the SQL of the project lives in this crate: if
//! a query appears in a different place, the boundary has broken.

mod mapping;
pub mod repositories;

use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("the migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("database backup error: {0}")]
    Io(#[from] std::io::Error),
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
        let existed = path.exists();
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        Self::connect(options, existed.then_some(path)).await
    }

    /// A temporary database for the tests. The same migration path as the real
    /// database.
    pub async fn in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("the in-memory URL is a constant")
            .foreign_keys(true);
        Self::connect(options, None).await
    }

    async fn connect(options: SqliteConnectOptions, backup_path: Option<&Path>) -> Result<Self> {
        // Only one connection: this is a desktop application for one user, and
        // thus the in-memory database of the tests does not disappear between
        // queries.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let db = Self { pool };
        if let Some(path) = backup_path {
            db.backup_before_migrations(path).await?;
        }
        db.migrate().await?;
        Ok(db)
    }

    /// Makes a consistent copy before the migrator changes a file database.
    ///
    /// The database can use WAL, so copying `library.db` would not include all
    /// of the committed pages. `VACUUM INTO` reads through SQLite and writes a
    /// complete database while this connection is open.
    async fn backup_before_migrations(&self, path: &Path) -> Result<()> {
        let Some(version) = self.pending_migration_version().await? else {
            return Ok(());
        };

        let backup = backup_path(path, version);
        let sql = format!("VACUUM INTO '{}'", sqlite_string(&backup));
        sqlx::query(&sql).execute(self.pool()).await?;
        prune_backups(path)?;
        Ok(())
    }

    /// Returns the first migration that the migrator still needs to apply.
    ///
    /// SQLx creates `_sqlx_migrations` during `run`, so a new file has no table
    /// yet. The empty applied set has the same meaning as an empty table.
    async fn pending_migration_version(&self) -> Result<Option<i64>> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS n FROM sqlite_master
             WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_one(self.pool())
        .await?;
        let has_table = row.get::<i64, _>("n") > 0;

        let applied: HashSet<i64> = if has_table {
            sqlx::query("SELECT version FROM _sqlx_migrations WHERE success = TRUE")
                .fetch_all(self.pool())
                .await?
                .iter()
                .map(|row| row.get("version"))
                .collect()
        } else {
            HashSet::new()
        };

        Ok(MIGRATOR
            .iter()
            .filter(|migration| migration.migration_type.is_up_migration())
            .map(|migration| migration.version)
            .find(|version| !applied.contains(version)))
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

fn backup_path(path: &Path, version: i64) -> std::path::PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("library.db");
    let prefix = format!("{name}.bak-{version}");
    let first = path.with_file_name(&prefix);
    if first.exists() {
        path.with_file_name(format!(
            "{prefix}-{}",
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ))
    } else {
        first
    }
}

fn sqlite_string(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

/// Keeps the three newest copies, including copies from an earlier failed
/// migration attempt.
fn prune_backups(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("library.db");
    let prefix = format!("{name}.bak-");
    let mut backups: Vec<(SystemTime, std::path::PathBuf)> = Vec::new();

    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let is_backup = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(&prefix));
        if metadata.is_file() && is_backup {
            backups.push((metadata.modified().unwrap_or(UNIX_EPOCH), entry.path()));
        }
    }

    backups.sort_by_key(|(modified, _)| *modified);
    let excess = backups.len().saturating_sub(3);
    for (_, backup) in backups.into_iter().take(excess) {
        fs::remove_file(backup)?;
    }
    Ok(())
}
