//! The games that the Steam family of the user can lend.
//!
//! This is not a store of the user either. A game of the family belongs to
//! another person: the user can play it today without buying it, and it stops
//! being playable the day that its owner leaves the family. Thus nothing here
//! is an entry, a link or a state: it is a fact about the game, as Luna is.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::matching::normalize;

/// What Steam says about the family of the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum FamilyGroup {
    /// The identifier of the family group, as text: Steam gives a 64-bit
    /// number, and JavaScript cannot hold it as a number with no loss.
    Member(String),
    /// Steam says that the user is in no family. It is an answer and not an
    /// error: the list is simply empty.
    NotMember,
}

/// A member of the family who owns a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyOwner {
    pub steam_id: String,
    /// The name of the profile. Absent when `GetPlayerSummaries` did not give
    /// it: the game can still be lent, only the mark cannot say by whom.
    pub name: Option<String>,
}

/// One game that the family can lend to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyApp {
    pub app_id: String,
    pub title: String,
    pub owners: Vec<FamilyOwner>,
    /// The IGDB record of the appid, when the identity pass found it. It is
    /// what marks a wish of GOG or of Epic whose title is not the one of Steam.
    pub igdb_id: Option<i64>,
}

/// What a row of the library says about the family: the appid, the title as
/// Steam writes it, the names of the owners that are known, and the address of
/// the page of the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyMark {
    pub app_id: String,
    pub title: String,
    pub owners: Vec<String>,
    pub url: String,
}

impl FamilyMark {
    /// The mark of a game of the family. The address comes from outside: the
    /// domain does not know where a store keeps its pages.
    pub fn of(app: &FamilyApp, url: String) -> Self {
        Self {
            app_id: app.app_id.clone(),
            title: app.title.clone(),
            owners: app
                .owners
                .iter()
                .filter_map(|owner| owner.name.clone())
                .collect(),
            url,
        }
    }
}

/// The list of the family, ready to answer "can the family lend this record".
///
/// Three keys, from the most certain to the least: the Steam appid of a copy of
/// the record, the IGDB record, and the normalised title. The title is an
/// exact match, as for Luna: a false mark makes the user not buy a game they
/// cannot play, and a miss only costs a look at Steam.
pub struct FamilyIndex<'a> {
    by_app: HashMap<&'a str, &'a FamilyApp>,
    by_igdb: HashMap<i64, &'a FamilyApp>,
    by_title: HashMap<String, &'a FamilyApp>,
}

impl<'a> FamilyIndex<'a> {
    pub fn new(apps: &'a [FamilyApp]) -> Self {
        let mut by_app = HashMap::with_capacity(apps.len());
        let mut by_igdb = HashMap::new();
        let mut by_title = HashMap::with_capacity(apps.len());
        for app in apps {
            by_app.entry(app.app_id.as_str()).or_insert(app);
            if let Some(igdb_id) = app.igdb_id {
                by_igdb.entry(igdb_id).or_insert(app);
            }
            let key = normalize(&app.title);
            // A title that normalises to nothing would match every record whose
            // title also normalises to nothing.
            if !key.is_empty() {
                // Two apps with the same normalised title are two editions of
                // one game, and the family can lend both: the first one is
                // kept, and the order of the list makes it the same each time.
                by_title.entry(key).or_insert(app);
            }
        }
        Self {
            by_app,
            by_igdb,
            by_title,
        }
    }

    /// The game of the family that is this record, if there is one.
    ///
    /// `app_ids` are the Steam appids of the live copies of the record, and
    /// `titles` are the title of the record and the titles of its copies.
    pub fn find<'s, 't>(
        &self,
        app_ids: impl IntoIterator<Item = &'s str>,
        igdb_id: Option<i64>,
        titles: impl IntoIterator<Item = &'t str>,
    ) -> Option<&'a FamilyApp> {
        app_ids
            .into_iter()
            .find_map(|app_id| self.by_app.get(app_id).copied())
            .or_else(|| igdb_id.and_then(|id| self.by_igdb.get(&id).copied()))
            .or_else(|| {
                titles.into_iter().find_map(|title| {
                    let key = normalize(title);
                    (!key.is_empty())
                        .then(|| self.by_title.get(&key).copied())
                        .flatten()
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(app_id: &str, title: &str, igdb_id: Option<i64>) -> FamilyApp {
        FamilyApp {
            app_id: app_id.to_owned(),
            title: title.to_owned(),
            owners: vec![FamilyOwner {
                steam_id: "76561190000000001".to_owned(),
                name: Some("Ana".to_owned()),
            }],
            igdb_id,
        }
    }

    #[test]
    fn the_appid_of_a_steam_copy_decides_before_the_title() {
        let apps = [
            app("1145360", "Hades", Some(113_112)),
            app("2", "Hades II", None),
        ];
        let index = FamilyIndex::new(&apps);

        // The copy says 1145360, and the record title says another game.
        let found = index.find(["1145360"], None, ["Hades II"]);
        assert_eq!(found.map(|a| a.app_id.as_str()), Some("1145360"));
    }

    #[test]
    fn a_gog_wish_is_found_by_its_igdb_record() {
        let apps = [app("292030", "The Witcher 3: Wild Hunt", Some(1942))];
        let index = FamilyIndex::new(&apps);

        // GOG names it another way, and the record carries the IGDB identity.
        let found = index.find([], Some(1942), ["The Witcher 3 Wild Hunt GOTY"]);
        assert_eq!(found.map(|a| a.app_id.as_str()), Some("292030"));
    }

    #[test]
    fn a_manual_wish_is_found_by_its_exact_title_and_not_by_a_similar_one() {
        let apps = [app("632470", "Disco Elysium", None)];
        let index = FamilyIndex::new(&apps);

        assert!(index.find([], None, ["DISCO ELYSIUM"]).is_some());
        assert!(index.find([], None, ["Disco Elysium 2"]).is_none());
        // An unknown appid and an unknown IGDB record do not stop the title.
        assert!(index.find(["1"], Some(7), ["Disco Elysium"]).is_some());
    }

    #[test]
    fn a_title_that_normalises_to_nothing_matches_nothing() {
        let apps = [app("1", "™", None)];
        let index = FamilyIndex::new(&apps);

        assert!(index.find([], None, ["®"]).is_none());
    }

    #[test]
    fn the_mark_keeps_only_the_names_that_are_known() {
        let mut lent = app("1", "Hades", None);
        lent.owners.push(FamilyOwner {
            steam_id: "76561190000000002".to_owned(),
            name: None,
        });

        let mark = FamilyMark::of(&lent, "https://example.test/1".to_owned());
        assert_eq!(mark.owners, ["Ana"]);
    }
}
