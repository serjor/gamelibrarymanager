//! What Amazon Luna includes with Prime.
//!
//! This is not a store of the user. Luna gives no copy: it gives a catalogue
//! that changes each month, and a game in it is a game that the user can play
//! today without buying it. Thus nothing here is an entry, a link or a state:
//! it is a fact about the game, and it stops being true when the catalogue
//! rotates.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::matching::normalize;

/// One game of the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LunaOffer {
    /// The Luna identifier, `amzn1.adg.product.…` or `amzn1.luna.product.…`.
    /// It is the identity of the offer: each tile has one, and the title
    /// changes with the language.
    pub product_id: String,
    pub title: String,
    /// The address of the page of the game, without the domain:
    /// `/game/<slug>/<ASIN>`, or `/game/<slug>` for the few games that Luna
    /// gives with no ASIN (Fortnite, on 2026-10-04).
    pub path: String,
    /// The Amazon identifier, when the address carries it.
    pub asin: Option<String>,
}

/// The catalogue that one answer of Luna gives.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LunaCatalog {
    /// The country that Luna says the catalogue is for. Amazon decides it from
    /// the connection and not from the request, thus it can differ from the
    /// country that the user selected.
    pub territory: Option<String>,
    pub offers: Vec<LunaOffer>,
}

/// What a row of the library says about Luna: the title as Luna writes it and
/// the address of its page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LunaMark {
    pub title: String,
    pub url: String,
}

/// The catalogue, ready to answer "is this record on Luna".
///
/// The match is by normalised title and it is exact. A similarity with a
/// threshold would say "On Luna" for a game and its remake, and a false "On
/// Luna" makes the user not buy a game they cannot play. A miss costs less: the
/// user looks at Luna, as they do today.
pub struct LunaIndex<'a> {
    by_title: HashMap<String, &'a LunaOffer>,
}

impl<'a> LunaIndex<'a> {
    pub fn new(offers: &'a [LunaOffer]) -> Self {
        let mut by_title = HashMap::with_capacity(offers.len());
        for offer in offers {
            let key = normalize(&offer.title);
            // A title that normalises to nothing would match every record
            // whose title also normalises to nothing.
            if key.is_empty() {
                continue;
            }
            // Two offers with the same normalised title are two editions of one
            // game on Luna, and both are playable: the first one is kept, and
            // the order of the catalogue makes it the same at each read.
            by_title.entry(key).or_insert(offer);
        }
        Self { by_title }
    }

    /// The offer of the first title of the record that is in the catalogue.
    ///
    /// The titles are the title of the record and the titles of its copies: a
    /// record named by IGDB and a copy named by the store can each be the one
    /// that Luna uses.
    pub fn find<'t>(&self, titles: impl IntoIterator<Item = &'t str>) -> Option<&'a LunaOffer> {
        titles.into_iter().find_map(|title| {
            let key = normalize(title);
            (!key.is_empty())
                .then(|| self.by_title.get(&key).copied())
                .flatten()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(title: &str) -> LunaOffer {
        LunaOffer {
            product_id: format!("amzn1.adg.product.{title}"),
            title: title.to_owned(),
            path: "/game/x".to_owned(),
            asin: None,
        }
    }

    #[test]
    fn the_match_ignores_trade_marks_case_and_packaging() {
        let offers = [
            offer("Ticket to Ride®"),
            offer("Tomb Raider Game of the Year"),
        ];
        let index = LunaIndex::new(&offers);

        assert_eq!(
            index.find(["Ticket to Ride"]).map(|o| o.title.as_str()),
            Some("Ticket to Ride®")
        );
        assert_eq!(
            index.find(["TOMB RAIDER"]).map(|o| o.title.as_str()),
            Some("Tomb Raider Game of the Year")
        );
    }

    #[test]
    fn a_remake_is_not_its_original() {
        let offers = [offer("Alan Wake Remastered")];
        let index = LunaIndex::new(&offers);

        assert_eq!(index.find(["Alan Wake"]), None);
        assert_eq!(index.find(["Alan Wake 2"]), None);
    }

    #[test]
    fn the_title_of_a_copy_can_be_the_one_that_matches() {
        let offers = [offer("Marvel's Guardians of the Galaxy")];
        let index = LunaIndex::new(&offers);

        assert!(
            index
                .find([
                    "Guardians of the Galaxy",
                    "Marvel's Guardians of the Galaxy"
                ])
                .is_some()
        );
    }

    #[test]
    fn a_title_that_normalises_to_nothing_matches_nothing() {
        let offers = [offer("™")];
        let index = LunaIndex::new(&offers);

        assert_eq!(index.find(["®"]), None);
    }
}
