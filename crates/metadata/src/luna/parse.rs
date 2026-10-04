//! The reading of the Luna answer, kept apart from the transport so that you
//! can test it with a recorded answer and with no network.
//!
//! The answer is a tree of widgets for a screen, not a list of games. This code
//! does not depend on where the grid is: it walks all of the widgets and takes
//! the `GAME_TILE`s. A new row or a new header on the page of Luna does not
//! change the result.

use std::collections::HashSet;

use domain::{LunaCatalog, LunaOffer};
use serde::Deserialize;

use crate::{MetadataError, Result};

#[derive(Deserialize)]
struct Page {
    #[serde(rename = "pageMemberGroups", default)]
    groups: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct Widget {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(rename = "presentationData", default)]
    presentation: Option<String>,
    #[serde(default)]
    actions: Vec<Action>,
    #[serde(default)]
    widgets: Vec<Widget>,
}

#[derive(Deserialize)]
struct Action {
    #[serde(default)]
    target: Option<String>,
}

/// `presentationData` is JSON inside a string.
#[derive(Deserialize)]
struct Tile {
    #[serde(rename = "gameId")]
    game_id: String,
    title: String,
    #[serde(rename = "hoverDetails", default)]
    hover: Option<Hover>,
}

#[derive(Deserialize)]
struct Hover {
    #[serde(rename = "productUrl", default)]
    product_url: Option<String>,
}

pub fn parse_catalog(body: &str) -> Result<LunaCatalog> {
    let page: Page = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("unreadable answer: {e}")))?;

    let mut catalog = LunaCatalog::default();
    let mut seen = HashSet::new();

    for group in page.groups.into_values() {
        // A group that does not have the shape of a group is not a game: it is
        // skipped, and the others are still read.
        if let Some(widgets) = group.get("widgets").cloned()
            && let Ok(widgets) = serde_json::from_value::<Vec<Widget>>(widgets)
        {
            collect(&widgets, &mut catalog, &mut seen);
        }
    }

    Ok(catalog)
}

/// The tiles in the order of the page, which is the order of Luna.
fn collect(widgets: &[Widget], catalog: &mut LunaCatalog, seen: &mut HashSet<String>) {
    for widget in widgets {
        if widget.kind.as_deref() == Some("GAME_TILE")
            && let Some((offer, territory)) = read_tile(widget)
            // The same game can appear in two rows of one screen.
            && seen.insert(offer.product_id.clone())
        {
            catalog.territory = catalog.territory.take().or(territory);
            catalog.offers.push(offer);
        }
        collect(&widget.widgets, catalog, seen);
    }
}

/// One game, or nothing if the tile does not say which game it is.
///
/// A tile with no page is skipped: the interface could not link to it, and
/// this code does not invent an address.
fn read_tile(widget: &Widget) -> Option<(LunaOffer, Option<String>)> {
    let tile: Tile = serde_json::from_str(widget.presentation.as_deref()?).ok()?;
    let (path, asin) = widget
        .actions
        .iter()
        .filter_map(|action| action.target.as_deref())
        .find_map(game_target)?;

    let title = tile.title.trim();
    if title.is_empty() {
        return None;
    }

    let territory = tile
        .hover
        .and_then(|hover| hover.product_url)
        .and_then(|url| territory(&url));

    Some((
        LunaOffer {
            product_id: tile.game_id,
            title: title.to_owned(),
            path: path.to_owned(),
            asin: asin.map(str::to_owned),
        },
        territory,
    ))
}

/// The page of a game, `/game/<slug>/<ASIN>` or `/game/<slug>`, and its ASIN.
fn game_target(target: &str) -> Option<(&str, Option<&str>)> {
    let mut parts = target.strip_prefix("/game/")?.split('/');
    parts.next().filter(|slug| is_slug(slug))?;
    let asin = match parts.next() {
        None => None,
        Some(asin) if is_asin(asin) => Some(asin),
        Some(_) => return None,
    };
    parts.next().is_none().then_some((target, asin))
}

/// Lower case letters, digits and hyphens: the slugs that Luna gives. Anything
/// else could take the link out of the page of the game.
fn is_slug(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Ten capitals and digits. A target with a different last part is not a game.
fn is_asin(text: &str) -> bool {
    text.len() == 10
        && text
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// The `territory` parameter of the address that starts the game.
fn territory(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("territory="))
        .filter(|code| code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase()))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_gives_the_page_and_the_asin() {
        assert_eq!(
            game_target("/game/hogwarts-legacy/B0CMQK7H1T"),
            Some(("/game/hogwarts-legacy/B0CMQK7H1T", Some("B0CMQK7H1T")))
        );
        // Fortnite comes like this on 2026-10-04.
        assert_eq!(
            game_target("/game/fortnite"),
            Some(("/game/fortnite", None))
        );
        assert_eq!(game_target("/browse"), None);
        assert_eq!(game_target("/game/"), None);
        assert_eq!(game_target("/game/slug/B0CMQK7H1T/extra"), None);
        assert_eq!(game_target("/game/slug/not-an-asin"), None);
        assert_eq!(game_target("/game/..%2F..%2Faccount"), None);
    }

    #[test]
    fn the_territory_comes_from_the_address_that_starts_the_game() {
        assert_eq!(
            territory("https://play.amazon.es/play?r=web&asin=B0DDMXR2WZ&territory=ES"),
            Some("ES".to_owned())
        );
        assert_eq!(territory("https://play.amazon.es/play?r=web"), None);
    }
}
