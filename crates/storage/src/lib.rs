//! Persistencia en SQLite. Todo el SQL del proyecto vive en este crate.
//!
//! Fase 2 del plan: esquema, migraciones y repositorios.

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("la migración {0} falló")]
    Migration(String),
}
