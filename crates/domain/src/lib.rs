//! Domain core: entities and rules. No network, no database, no Tauri.
//!
//! If a dependency on IO appears here one day, the architecture has broken. The
//! `architecture` step of CI runs `cargo tree -p domain` and fails if it finds
//! reqwest, sqlx or tauri in the tree.

pub mod library;
pub mod matching;
pub mod model;
pub mod ports;
pub mod prices;

pub use library::{Game, GameLink, LinkMethod, StoreAccount, StoreEntry, UserState};
pub use matching::{Candidate, MatchDecision, ScoredCandidate};
pub use model::{
    ConnectorState, EntryKind, GameId, PlayStatus, StoreAccountId, StoreEntryId, StoreId,
};
pub use ports::{AuthContext, ClientCredentials, ConnectorError, StoreConnector, StoreSession};
pub use prices::{Deal, GamePrices, Money};
