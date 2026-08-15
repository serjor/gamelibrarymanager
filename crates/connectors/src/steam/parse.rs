//! The reading of the Steam answers, kept apart from the transport so that you
//! can test it with recorded answers and with no network.

use std::collections::HashMap;

use domain::{ConnectorError, EntryKind, StoreAccountId, StoreEntry, StoreEntryId, StoreId};
use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Deserialize)]
struct Envelope<T> {
    response: T,
}

#[derive(Deserialize)]
struct OwnedGames {
    #[serde(default)]
    games: Vec<OwnedGame>,
}

#[derive(Deserialize)]
struct OwnedGame {
    appid: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    playtime_forever: i64,
    #[serde(default)]
    rtime_last_played: Option<i64>,
}

#[derive(Deserialize)]
struct Wishlist {
    #[serde(default)]
    items: Vec<WishlistItem>,
}

#[derive(Deserialize)]
struct WishlistItem {
    appid: i64,
    #[serde(default)]
    date_added: Option<i64>,
}

/// The library. A profile that keeps its game details private gives back an
/// empty object and not an error, thus you must find that condition here: the
/// user must know that the problem is the privacy and not their key.
pub fn parse_owned(
    body: &str,
    account_id: StoreAccountId,
) -> Result<Vec<StoreEntry>, ConnectorError> {
    let envelope: Envelope<serde_json::Value> = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("unreadable answer: {e}")))?;

    if envelope.response.as_object().is_none_or(|o| o.is_empty()) {
        return Err(ConnectorError::Private);
    }

    let parsed: Envelope<OwnedGames> = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("unreadable answer: {e}")))?;

    Ok(parsed
        .response
        .games
        .into_iter()
        .map(|game| StoreEntry {
            id: StoreEntryId::new(),
            account_id,
            store: StoreId::Steam,
            store_app_id: game.appid.to_string(),
            kind: EntryKind::Owned,
            title: game
                .name
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("Steam {}", game.appid)),
            playtime_minutes: Some(game.playtime_forever),
            acquired_at: None,
            cover_url: Some(cover_url(game.appid)),
            store_url: Some(store_url(game.appid)),
            raw: serde_json::json!({
                "appid": game.appid,
                "playtime_forever": game.playtime_forever,
                "rtime_last_played": game.rtime_last_played,
            }),
        })
        .collect())
}

/// The wished-for games. This endpoint gives back only appids and dates: the
/// titles are requested separately, and if they do not come, phase 4 still
/// resolves the record by appid.
pub fn parse_wishlist(
    body: &str,
    account_id: StoreAccountId,
) -> Result<Vec<StoreEntry>, ConnectorError> {
    let parsed: Envelope<Wishlist> = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("unreadable answer: {e}")))?;

    Ok(parsed
        .response
        .items
        .into_iter()
        .map(|item| StoreEntry {
            id: StoreEntryId::new(),
            account_id,
            store: StoreId::Steam,
            store_app_id: item.appid.to_string(),
            kind: EntryKind::Wishlist,
            title: format!("Steam {}", item.appid),
            playtime_minutes: None,
            acquired_at: item
                .date_added
                .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok()),
            cover_url: Some(cover_url(item.appid)),
            store_url: Some(store_url(item.appid)),
            raw: serde_json::json!({ "appid": item.appid, "date_added": item.date_added }),
        })
        .collect())
}

/// The capsule image of the game. It comes from the appid and costs no request:
/// Steam always gives it at that address, and it is the image by which a person
/// recognises a game of their library.
fn cover_url(app_id: i64) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg")
}

/// Its page in the store, so that you can go and see which game it is.
fn store_url(app_id: i64) -> String {
    format!("https://store.steampowered.com/app/{app_id}")
}

/// The name of the account, so that the interface can show something more
/// readable than a steamid.
pub fn parse_player_name(body: &str, steam_id: &str) -> Result<Option<String>, ConnectorError> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("unreadable answer: {e}")))?;

    let players = value
        .get("response")
        .and_then(|r| r.get("players"))
        .and_then(|p| p.as_array())
        .ok_or_else(|| ConnectorError::Unexpected("response.players is absent".to_owned()))?;

    // A valid key with a steamid that does not exist gives back an empty list.
    if players.is_empty() {
        return Err(ConnectorError::Unexpected(format!(
            "Steam does not know the steamid {steam_id}"
        )));
    }

    Ok(players
        .first()
        .and_then(|p| p.get("personaname"))
        .and_then(|n| n.as_str())
        .map(str::to_owned))
}

/// The titles from the API of the store. It gives the best result that it can:
/// the titles that do not come keep their placeholder.
pub fn parse_app_details(body: &str) -> HashMap<String, String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return HashMap::new();
    };
    let Some(object) = value.as_object() else {
        return HashMap::new();
    };

    object
        .iter()
        .filter_map(|(app_id, entry)| {
            let name = entry.get("data")?.get("name")?.as_str()?;
            Some((app_id.clone(), name.to_owned()))
        })
        .collect()
}
