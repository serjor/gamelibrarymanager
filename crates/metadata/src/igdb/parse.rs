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
    uid: String,
    game: i64,
}

#[derive(Deserialize)]
struct RawGame {
    id: i64,
    name: String,
    #[serde(default)]
    slug: Option<String>,
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

/// Los cruces de un lote, indexados por el identificador de la tienda.
///
/// Un `uid` puede venir repetido —IGDB registra la misma copia bajo varias
/// fichas cuando hay ediciones— y entonces gana la primera. Devolver dos fichas
/// para un identificador que se pidió como exacto sería mentir sobre lo que es
/// exacto, y quien llama no tiene con qué desempatar.
pub fn parse_external_games(body: &str) -> Result<Vec<(String, i64)>> {
    let parsed: Vec<ExternalGame> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("respuesta ilegible: {e}")))?;
    Ok(parsed.into_iter().map(|e| (e.uid, e.game)).collect())
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
            // En la cola de revisión la portada es una miniatura para
            // distinguir de un vistazo, no una carátula: basta la talla chica.
            cover_url: game.cover.map(|cover| cover_url(&cover, "t_cover_small")),
            slug: game.slug,
        })
        .collect())
}

/// IGDB sirve las portadas por plantilla, y la talla va en la propia dirección.
fn cover_url(cover: &Cover, size: &str) -> String {
    format!(
        "https://images.igdb.com/igdb/image/upload/{size}/{}.jpg",
        cover.image_id
    )
}

pub fn parse_game(body: &str) -> Result<Option<GameMetadata>> {
    let parsed: Vec<RawGame> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("respuesta ilegible: {e}")))?;

    Ok(parsed.into_iter().next().map(|game| GameMetadata {
        igdb_id: game.id,
        name: game.name,
        summary: game.summary,
        // t_cover_big es el tamaño que se ve bien en la rejilla de la
        // biblioteca sin disparar el peso.
        cover_url: game.cover.map(|cover| cover_url(&cover, "t_cover_big")),
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
