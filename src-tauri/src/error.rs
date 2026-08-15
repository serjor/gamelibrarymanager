//! One error type towards the interface. The messages are made to be read on
//! the screen, not to debug.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("there are no credentials kept for this account: connect it again")]
    MissingCredential,
    #[error(transparent)]
    Storage(#[from] storage::StorageError),
    #[error(transparent)]
    Secrets(#[from] secrets::SecretsError),
    #[error(transparent)]
    Connector(#[from] domain::ConnectorError),
    #[error(transparent)]
    Metadata(#[from] metadata::MetadataError),
    #[error(
        "the IGDB credentials are absent: without them there is no metadata and there are no unified records"
    )]
    MissingIgdbCredentials,
    #[error("the ITAD key is absent: without it there are no prices of the wished-for games")]
    MissingItadCredentials,
    #[error("unreadable internal data: {0}")]
    Serde(#[from] serde_json::Error),
}

// Tauri must serialise the error to send it to the frontend. It sends the
// message and nothing else: an error is not a place to show internal state.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
