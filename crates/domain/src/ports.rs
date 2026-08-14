//! Los contratos que el dominio exige y los adaptadores cumplen. Aquí no hay
//! ninguna implementación: es lo que permite que GOG y Epic entren en las fases
//! 6 y 7 sin tocar una línea de esta carpeta.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::library::StoreEntry;
use crate::model::{StoreAccountId, StoreId};

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("credenciales inválidas o caducadas")]
    Unauthorized,
    #[error("la tienda limitó las peticiones")]
    RateLimited,
    #[error("la biblioteca es privada y las credenciales no dan acceso")]
    Private,
    #[error("no se pudo contactar con la tienda: {0}")]
    Transport(String),
    #[error("la tienda respondió de forma inesperada: {0}")]
    Unexpected(String),
}

/// Credenciales de *cliente* de una tienda: identifican a la aplicación, no al
/// usuario.
///
/// Las aporta el usuario al conectar la cuenta, igual que la clave de Steam.
/// No es una elección estética: GOG no permite registrar un cliente propio, así
/// que la única forma de no llevar un secreto dentro del binario es que el par
/// entre por la misma puerta que las demás claves y viva en el almacén.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCredentials {
    pub client_id: String,
    pub client_secret: String,
}

/// Lo que el usuario aporta para conectar una cuenta. Steam usa clave propia;
/// GOG y Epic usarán el código que devuelve su propio formulario de login.
#[derive(Debug, Clone)]
pub enum AuthContext {
    /// Clave de API del propio usuario. En Steam es además lo que da acceso a
    /// su biblioteca privada sin abrir el perfil.
    ApiKey { key: String, account_ref: String },
    /// Código de autorización devuelto por la página de login de la tienda,
    /// junto con el cliente ante el que se pidió. Los dos hacen falta para
    /// canjearlo: el código solo vale para el cliente que lo originó.
    AuthCode {
        code: String,
        client: ClientCredentials,
    },
    /// Material guardado en una sesión anterior.
    Stored { credential: String },
}

/// Sesión abierta contra una tienda.
///
/// `credential` es opaco: solo el conector que lo emitió sabe interpretarlo. El
/// resto del sistema lo trata como un bloque que va al almacén de secretos y
/// vuelve tal cual. Nunca se escribe en la base de datos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSession {
    pub store: StoreId,
    pub account_ref: String,
    pub display_name: Option<String>,
    pub credential: String,
    pub expires_at: Option<OffsetDateTime>,
}

/// Un conector lee. No instala, no descarga y no lanza nada.
#[async_trait]
pub trait StoreConnector: Send + Sync {
    fn id(&self) -> StoreId;

    async fn authenticate(&self, ctx: &AuthContext) -> Result<StoreSession, ConnectorError>;

    async fn owned(
        &self,
        session: &StoreSession,
        account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError>;

    async fn wishlist(
        &self,
        session: &StoreSession,
        account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError>;
}
