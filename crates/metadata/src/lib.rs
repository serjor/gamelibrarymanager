//! Proveedores de metadatos. IGDB en la fase 4, con límite de 4 req/s y caché
//! permanente por identificador: un juego se consulta una vez en la vida.

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("límite de peticiones alcanzado")]
    RateLimited,
    #[error("el proveedor respondió de forma inesperada: {0}")]
    Unexpected(String),
}
