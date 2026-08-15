//! One repository for each aggregate. Each one owns its tables and no
//! repository writes in the tables of another.

mod connector_state;
mod game;
mod game_link;
mod library;
mod match_candidate;
mod price;
mod store_account;
mod store_entry;
mod user_state;

pub use connector_state::ConnectorStateRepository;
pub use game::GameRepository;
pub use game_link::GameLinkRepository;
pub use library::{LibraryRepository, LibraryRow};
pub use match_candidate::MatchCandidateRepository;
pub use price::{PriceRepository, PriceRow, PriceTarget};
pub use store_account::StoreAccountRepository;
pub use store_entry::StoreEntryRepository;
pub use user_state::UserStateRepository;
