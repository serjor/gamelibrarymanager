//! The contracts that the domain demands and the adapters obey. There is no
//! implementation here: this is what lets GOG and Epic come in at phases 6 and
//! 7 without a change to one line of this directory.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::library::StoreEntry;
use crate::model::{StoreAccountId, StoreId};

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    // Every message says what to do and not only what failed. A store that
    // stops answering is the most common failure of the whole application, and
    // "invalid credentials" leaves the user with nowhere to go.
    #[error("invalid or expired credentials: connect the account again")]
    Unauthorized,
    #[error("the store limited the requests")]
    RateLimited,
    #[error("the library is private and the credentials do not give access")]
    Private,
    #[error("could not contact the store: {0}")]
    Transport(String),
    #[error("the store gave an unexpected answer: {0}")]
    Unexpected(String),
}

/// The *client* credentials of a store: they identify the application, not the
/// user.
///
/// The user supplies them when they connect the account, as with the Steam key.
/// This is not a decision about style: GOG does not let you register a client
/// of your own, thus the only way to keep a secret out of the binary is to let
/// the pair come in through the same door as the other keys and live in the
/// store of secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCredentials {
    pub client_id: String,
    pub client_secret: String,
}

/// What the user supplies to connect an account. Steam uses a key of its own;
/// GOG and Epic will use the code that their own login form gives back.
#[derive(Debug, Clone)]
pub enum AuthContext {
    /// The API key of the user. In Steam it is also what gives access to their
    /// private library without they make the profile public.
    ApiKey { key: String, account_ref: String },
    /// The authorisation code that the login page of the store gave back,
    /// together with the client that asked for it. You need the two to exchange
    /// it: the code is applicable only to the client that caused it.
    AuthCode {
        code: String,
        client: ClientCredentials,
    },
    /// Material kept from an earlier session.
    Stored { credential: String },
}

/// An open session with a store.
///
/// `credential` is opaque: only the connector that issued it can read it. The
/// remainder of the system holds it as a block that goes to the store of
/// secrets and comes back unchanged. It is never written to the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSession {
    pub store: StoreId,
    pub account_ref: String,
    pub display_name: Option<String>,
    pub credential: String,
    pub expires_at: Option<OffsetDateTime>,
}

/// A connector reads. It does not install, it does not download and it does not
/// start anything.
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
