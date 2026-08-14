//! Persistencia en SQLite. Todo el SQL del proyecto vive en este crate: si
//! aparece una consulta en cualquier otro sitio, la frontera se ha roto.

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
    #[error("error de base de datos: {0}")]
    Database(#[from] sqlx::Error),
    #[error("la migración falló: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("valor inesperado en la columna {column}: {value}")]
    Corrupt { column: &'static str, value: String },
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Abre —o crea— la base de datos del usuario y la deja migrada.
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        Self::connect(options).await
    }

    /// Base efímera para los tests. Misma ruta de migración que la real.
    pub async fn in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("la URL en memoria es constante")
            .foreign_keys(true);
        Self::connect(options).await
    }

    async fn connect(options: SqliteConnectOptions) -> Result<Self> {
        // Una sola conexión: es una app de escritorio mono-usuario y así la base
        // en memoria de los tests no se evapora entre consultas.
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

    /// Revierte todas las migraciones. Existe para que el esquema tenga vuelta
    /// atrás demostrable, no porque la app lo use en funcionamiento normal.
    pub async fn undo_all(&self) -> Result<()> {
        MIGRATOR.undo(&self.pool, 0).await?;
        Ok(())
    }

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
