//! Lectura de las respuestas de Steam, separada del transporte para poder
//! probarla con respuestas grabadas y sin red.

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

/// Biblioteca. Un perfil con los detalles de juego en privado devuelve un
/// objeto vacío en vez de un error, así que hay que distinguirlo a mano: el
/// usuario merece saber que el problema es la privacidad y no su clave.
pub fn parse_owned(
    body: &str,
    account_id: StoreAccountId,
) -> Result<Vec<StoreEntry>, ConnectorError> {
    let envelope: Envelope<serde_json::Value> = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("respuesta ilegible: {e}")))?;

    if envelope.response.as_object().is_none_or(|o| o.is_empty()) {
        return Err(ConnectorError::Private);
    }

    let parsed: Envelope<OwnedGames> = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("respuesta ilegible: {e}")))?;

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
            raw: serde_json::json!({
                "appid": game.appid,
                "playtime_forever": game.playtime_forever,
                "rtime_last_played": game.rtime_last_played,
            }),
        })
        .collect())
}

/// Deseados. Este endpoint solo devuelve appids y fechas: los títulos se piden
/// aparte, y si no llegan la ficha se resuelve igualmente por appid en la
/// fase 4.
pub fn parse_wishlist(
    body: &str,
    account_id: StoreAccountId,
) -> Result<Vec<StoreEntry>, ConnectorError> {
    let parsed: Envelope<Wishlist> = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("respuesta ilegible: {e}")))?;

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
            raw: serde_json::json!({ "appid": item.appid, "date_added": item.date_added }),
        })
        .collect())
}

/// Nombre de la cuenta, para poder enseñar algo más humano que un steamid.
pub fn parse_player_name(body: &str, steam_id: &str) -> Result<Option<String>, ConnectorError> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("respuesta ilegible: {e}")))?;

    let players = value
        .get("response")
        .and_then(|r| r.get("players"))
        .and_then(|p| p.as_array())
        .ok_or_else(|| ConnectorError::Unexpected("falta response.players".to_owned()))?;

    // Una clave válida con un steamid inexistente devuelve la lista vacía.
    if players.is_empty() {
        return Err(ConnectorError::Unexpected(format!(
            "Steam no conoce el steamid {steam_id}"
        )));
    }

    Ok(players
        .first()
        .and_then(|p| p.get("personaname"))
        .and_then(|n| n.as_str())
        .map(str::to_owned))
}

/// Títulos desde la API de la tienda. Es best-effort por definición: lo que no
/// venga se queda con el marcador de posición.
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
