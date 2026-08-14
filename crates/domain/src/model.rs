use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Las tiendas que el sistema sabe leer. Añadir una variante obliga a revisar
/// todo el emparejamiento, que es exactamente la intención.
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

/// Una entrada de tienda es propiedad o deseo. Nada más: son los dos únicos
/// listados que los conectores saben leer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Owned,
    Wishlist,
}

/// Estado que el usuario asigna a un juego. Es el único dato que ninguna
/// sincronización puede sobrescribir.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_time_ordered() {
        let first = GameId::new();
        let second = GameId::new();
        assert!(
            first.as_uuid() < second.as_uuid(),
            "UUIDv7 debe ser ordenable por tiempo"
        );
    }

    #[test]
    fn store_ids_serialize_as_stable_strings() {
        assert_eq!(serde_json::to_string(&StoreId::Gog).unwrap(), "\"gog\"");
        assert_eq!(StoreId::Steam.as_str(), "steam");
    }
}
