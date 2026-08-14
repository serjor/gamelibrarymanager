//! Conectores de tienda: solo autenticación y listado. Nunca descargas.
//!
//! Fase 3 del plan introduce el trait `StoreConnector` y la implementación de
//! Steam; GOG y Epic entran en las fases 6 y 7 sin tocar el dominio.

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("credenciales inválidas o caducadas")]
    Unauthorized,
    #[error("la tienda respondió de forma inesperada: {0}")]
    Unexpected(String),
}
