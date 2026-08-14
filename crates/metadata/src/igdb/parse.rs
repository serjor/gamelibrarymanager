//! Lectura de las respuestas de IGDB, separada del transporte para poder
//! probarla con respuestas grabadas y sin red.

use domain::Candidate;
use serde::Deserialize;
use time::OffsetDateTime;

use super::{GameMetadata, IgdbToken};
use crate::{MetadataError, Result};

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct ExternalGame {
    game: i64,
}

#[derive(Deserialize)]
struct RawGame {
    id: i64,
    name: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    first_release_date: Option<i64>,
    #[serde(default)]
    alternative_names: Vec<AlternativeName>,
    #[serde(default)]
    cover: Option<Cover>,
    #[serde(default)]
    genres: Vec<Genre>,
}

#[derive(Deserialize)]
struct Genre {
    name: String,
}

#[derive(Deserialize)]
struct AlternativeName {
    name: String,
}

#[derive(Deserialize)]
struct Cover {
    image_id: String,
}

pub fn parse_token(body: &str, now: OffsetDateTime) -> Result<IgdbToken> {
    let parsed: TokenResponse = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("token ilegible: {e}")))?;
    Ok(IgdbToken {
        access_token: parsed.access_token,
        expires_at: now.unix_timestamp() + parsed.expires_in,
    })
}

pub fn parse_external_game(body: &str) -> Result<Option<i64>> {
    let parsed: Vec<ExternalGame> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("respuesta ilegible: {e}")))?;
    Ok(parsed.first().map(|e| e.game))
}

pub fn parse_candidates(body: &str) -> Result<Vec<Candidate>> {
    let parsed: Vec<RawGame> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("respuesta ilegible: {e}")))?;

    Ok(parsed
        .into_iter()
        .map(|game| Candidate {
            igdb_id: game.id,
            name: game.name,
            alternative_names: game
                .alternative_names
                .into_iter()
                .map(|alt| alt.name)
                .collect(),
            release_year: game.first_release_date.and_then(year_of),
        })
        .collect())
}

pub fn parse_game(body: &str) -> Result<Option<GameMetadata>> {
    let parsed: Vec<RawGame> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("respuesta ilegible: {e}")))?;

    Ok(parsed.into_iter().next().map(|game| GameMetadata {
        igdb_id: game.id,
        name: game.name,
        summary: game.summary,
        // IGDB sirve las portadas por plantilla; t_cover_big es el tamaño que
        // se ve bien en una rejilla sin disparar el peso.
        cover_url: game.cover.map(|cover| {
            format!(
                "https://images.igdb.com/igdb/image/upload/t_cover_big/{}.jpg",
                cover.image_id
            )
        }),
        released_at: game
            .first_release_date
            .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok()),
        genres: game.genres.into_iter().map(|genre| genre.name).collect(),
    }))
}

fn year_of(timestamp: i64) -> Option<i32> {
    OffsetDateTime::from_unix_timestamp(timestamp)
        .ok()
        .map(|date| date.year())
}
