//! The metadata providers.
//!
//! IGDB belongs to Twitch, and its developer agreement **prohibits** a client
//! secret inside a desktop application: the only method that does not need a
//! server is that each user registers an application of their own in Twitch.
//! Thus the credentials come in as a parameter and there is no constant here.
//!
//! ITAD comes in through the same door and for the same reason: its key belongs
//! to the user, it lives in the store of secrets and it comes in as a parameter.
//! What it gives is not a record but a price, which is what turns a wishlist
//! into a decision to buy.

pub mod igdb;
pub mod itad;
mod rate_limit;

pub use igdb::IgdbClient;
pub use itad::ItadClient;

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    // One error for the two providers, thus the message cannot name IGDB: a
    // user who makes a mistake with the ITAD key would read that a different
    // thing is incorrect.
    #[error("the credentials of the provider are not valid")]
    Unauthorized,
    #[error("the request limit is reached")]
    RateLimited,
    #[error("could not contact the provider: {0}")]
    Transport(String),
    #[error("the provider gave an unexpected answer: {0}")]
    Unexpected(String),
}

pub type Result<T> = std::result::Result<T, MetadataError>;
