//! Un único tipo de error hacia la UI. Los mensajes están pensados para
//! leerse en pantalla, no para depurar.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("no hay credenciales guardadas para esta cuenta: vuelve a conectarla")]
    MissingCredential,
    #[error(transparent)]
    Storage(#[from] storage::StorageError),
    #[error(transparent)]
    Secrets(#[from] secrets::SecretsError),
    #[error(transparent)]
    Connector(#[from] domain::ConnectorError),
}

// Tauri necesita serializar el error para cruzarlo al frontend. Se manda el
// mensaje y nada más: un error no es sitio para volcar estado interno.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
