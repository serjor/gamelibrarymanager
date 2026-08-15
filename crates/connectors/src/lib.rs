//! Conectores de tienda: solo autenticación y listado. Nunca descargas.
//!
//! Cada tienda implementa `domain::StoreConnector`. El dominio no sabe que
//! existe HTTP y los conectores no saben que existe una base de datos.

pub mod epic;
pub mod gog;
pub mod steam;

pub use epic::EpicConnector;
pub use gog::GogConnector;
pub use steam::SteamConnector;
