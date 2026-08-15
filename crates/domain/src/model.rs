use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The stores that the system can read. If you add a variant, you must examine
/// all of the matching code. That is the intention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreId {
    Steam,
    Gog,
    Epic,
}

impl StoreId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Steam => "steam",
            Self::Gog => "gog",
            Self::Epic => "epic",
        }
    }
}

/// Whether a store connector is on, and what went wrong the last time it ran.
///
/// Epic is the reason this exists. Its authentication rests on the private API
/// of its own launcher, so it can stop working on a day nobody chose, and one
/// broken store cannot be allowed to make the application useless. Turning the
/// connector off leaves the rest of the library exactly as it was.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectorState {
    pub store: StoreId,
    pub enabled: bool,
    pub last_error: Option<String>,
}

/// A store entry is owned or wished for. Nothing else: these are the only two
/// lists that the connectors can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Owned,
    Wishlist,
}

/// The status that the user gives to a game. It is the only data that no
/// synchronisation can overwrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayStatus {
    Backlog,
    Playing,
    Finished,
    Abandoned,
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub const fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

id_type!(GameId);
id_type!(StoreEntryId);
id_type!(StoreAccountId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_time_ordered() {
        let first = GameId::new();
        let second = GameId::new();
        assert!(
            first.as_uuid() < second.as_uuid(),
            "a UUIDv7 must sort by time"
        );
    }

    #[test]
    fn store_ids_serialize_as_stable_strings() {
        assert_eq!(serde_json::to_string(&StoreId::Gog).unwrap(), "\"gog\"");
        assert_eq!(StoreId::Steam.as_str(), "steam");
    }
}
