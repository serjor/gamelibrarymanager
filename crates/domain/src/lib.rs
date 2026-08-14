//! Núcleo de dominio: entidades y reglas. Sin red, sin base de datos, sin Tauri.
//!
//! Si algún día aparece aquí una dependencia de IO, la arquitectura se ha roto.
//! El paso `arquitectura` de CI ejecuta `cargo tree -p domain` y falla si
//! encuentra reqwest, sqlx o tauri en el árbol.

pub mod model;

pub use model::{EntryKind, GameId, PlayStatus, StoreEntryId, StoreId};
