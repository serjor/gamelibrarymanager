//! Almacén de secretos. Aquí van las claves de API y los tokens de tienda, y
//! solo aquí: nunca a SQLite, nunca a un fichero de configuración, nunca a un
//! log.
//!
//! Nada de `tauri-plugin-stronghold`: desaparece en Tauri v3.

mod encrypted_file;
mod keyring_store;

pub use encrypted_file::EncryptedFileStore;
pub use keyring_store::KeyringStore;

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("no hay almacén de secretos disponible en este sistema")]
    Unavailable,
    #[error("la contraseña no abre el almacén")]
    WrongPassphrase,
    #[error("el almacén está corrupto: {0}")]
    Corrupt(String),
    #[error("error del almacén del sistema: {0}")]
    Backend(String),
    #[error("error de entrada/salida: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SecretsError>;

/// Guardar y recuperar secretos por nombre. Deliberadamente sin `list`: nadie
/// necesita enumerar credenciales y no tenerlo evita la tentación.
pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// Qué almacén puede usarse en esta máquina.
///
/// En Linux sin secret-service —contenedores, escritorios mínimos, sesiones
/// remotas— el keyring nativo no existe, y descubrirlo al guardar la primera
/// clave es la peor forma de enterarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// Keyring del sistema operativo: sin contraseña que recordar.
    Keyring,
    /// Fichero cifrado: exige una contraseña al usuario.
    Passphrase,
}

/// Comprueba de verdad si el keyring responde, con un secreto de usar y tirar.
/// Preguntar por la plataforma no vale: dos Linux iguales se comportan distinto
/// según qué haya arrancado en la sesión.
pub fn detect(service: &str) -> Backend {
    let probe = KeyringStore::new(service);
    let key = "__probe__";
    match probe.set(key, "ok").and_then(|()| probe.get(key)) {
        Ok(Some(value)) if value == "ok" => {
            let _ = probe.delete(key);
            Backend::Keyring
        }
        _ => Backend::Passphrase,
    }
}
