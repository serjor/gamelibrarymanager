//! Store connectors: only authentication and lists. Never downloads.
//!
//! Each store obeys `domain::StoreConnector`. The domain does not know that HTTP
//! exists, and the connectors do not know that a database exists.

pub mod epic;
pub mod gog;
pub mod steam;

pub use epic::EpicConnector;
pub use gog::GogConnector;
pub use steam::SteamConnector;
