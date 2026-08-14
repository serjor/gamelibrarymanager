//! Un repositorio por agregado. Cada uno es dueño de sus tablas y nadie escribe
//! en las tablas de otro.

mod game;
mod game_link;
mod store_account;
mod store_entry;
mod user_state;

pub use game::GameRepository;
pub use game_link::GameLinkRepository;
pub use store_account::StoreAccountRepository;
pub use store_entry::StoreEntryRepository;
pub use user_state::UserStateRepository;
