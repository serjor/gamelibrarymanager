//! Conectores de tienda: solo autenticación y listado. Nunca descargas.
//!
//! Cada tienda implementa `domain::StoreConnector`. El dominio no sabe que
//! existe HTTP y los conectores no saben que existe una base de datos.

pub mod steam;

pub use steam::SteamConnector;
