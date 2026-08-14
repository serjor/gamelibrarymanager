//! Almacén de secretos sobre el keyring nativo del sistema operativo.
//!
//! Nada de `tauri-plugin-stronghold`: desaparece en Tauri v3. En Linux sin
//! secret-service se cae a fichero cifrado con passphrase (fase 3).

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("no hay almacén de secretos disponible en este sistema")]
    Unavailable,
    #[error("no existe el secreto {0}")]
    NotFound(String),
}
