//! Proveedores de metadatos.
//!
//! IGDB es de Twitch y su acuerdo de desarrollador **prohíbe** empotrar el
//! client secret en una aplicación de escritorio: la única salida sin montar un
//! servidor es que cada usuario registre su propia aplicación en Twitch. Por eso
//! las credenciales entran por parámetro y no hay ninguna constante aquí.

pub mod igdb;

pub use igdb::IgdbClient;

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("las credenciales de IGDB no son válidas")]
    Unauthorized,
    #[error("límite de peticiones alcanzado")]
    RateLimited,
    #[error("no se pudo contactar con el proveedor: {0}")]
    Transport(String),
    #[error("el proveedor respondió de forma inesperada: {0}")]
    Unexpected(String),
}

pub type Result<T> = std::result::Result<T, MetadataError>;
