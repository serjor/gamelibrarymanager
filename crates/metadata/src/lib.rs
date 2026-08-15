//! Proveedores de metadatos.
//!
//! IGDB es de Twitch y su acuerdo de desarrollador **prohíbe** empotrar el
//! client secret en una aplicación de escritorio: la única salida sin montar un
//! servidor es que cada usuario registre su propia aplicación en Twitch. Por eso
//! las credenciales entran por parámetro y no hay ninguna constante aquí.
//!
//! ITAD entra por la misma puerta y por el mismo motivo: su clave es del
//! usuario, vive en el almacén de secretos y llega por parámetro. Lo que aporta
//! no es una ficha sino un precio, que es lo que convierte una lista de deseados
//! en una decisión de compra.

pub mod igdb;
pub mod itad;
mod rate_limit;

pub use igdb::IgdbClient;
pub use itad::ItadClient;

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    // Un solo error para los dos proveedores, así que el mensaje no puede
    // nombrar a IGDB: quien se equivoque con la clave de ITAD leería que lo que
    // está mal es otra cosa.
    #[error("las credenciales del proveedor no son válidas")]
    Unauthorized,
    #[error("límite de peticiones alcanzado")]
    RateLimited,
    #[error("no se pudo contactar con el proveedor: {0}")]
    Transport(String),
    #[error("el proveedor respondió de forma inesperada: {0}")]
    Unexpected(String),
}

pub type Result<T> = std::result::Result<T, MetadataError>;
