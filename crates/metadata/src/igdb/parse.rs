//! The reading of the IGDB answers, kept apart from the transport so that you
//! can test it with recorded answers and with no network.

use domain::{Candidate, TimeToBeat};
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
struct RawTimeToBeat {
    game_id: i64,
    #[serde(default)]
    hastily: Option<i64>,
    #[serde(default)]
    normally: Option<i64>,
    #[serde(default)]
    completely: Option<i64>,
    #[serde(default)]
    count: Option<i64>,
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
        .map_err(|e| MetadataError::Unexpected(format!("unreadable token: {e}")))?;
    Ok(IgdbToken {
        access_token: parsed.access_token,
        expires_at: now.unix_timestamp() + parsed.expires_in,
    })
}

/// The joins of one batch, indexed by the identifier of the store.
///
/// A `uid` can come more than once — IGDB records the same copy under more than
/// one record when there are editions — and then the first one wins. To give
/// back two records for an identifier that was requested as exact would be false
/// about what is exact, and the caller has no data to break the tie.
pub fn parse_external_games(body: &str) -> Result<Vec<(String, i64)>> {
    let parsed: Vec<ExternalGame> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("unreadable answer: {e}")))?;
    Ok(parsed.into_iter().map(|e| (e.uid, e.game)).collect())
}

pub fn parse_candidates(body: &str) -> Result<Vec<Candidate>> {
    let parsed: Vec<RawGame> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("unreadable answer: {e}")))?;

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
            // In the review queue the cover is a small image to tell candidates
            // apart quickly, not a full cover: the small size is sufficient.
            cover_url: game.cover.map(|cover| cover_url(&cover, "t_cover_small")),
            slug: game.slug,
        })
        .collect())
}

/// IGDB gives the covers through a template, and the size goes in the address.
fn cover_url(cover: &Cover, size: &str) -> String {
    format!(
        "https://images.igdb.com/igdb/image/upload/{size}/{}.jpg",
        cover.image_id
    )
}

pub fn parse_game(body: &str) -> Result<Option<GameMetadata>> {
    let parsed: Vec<RawGame> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("unreadable answer: {e}")))?;

    Ok(parsed.into_iter().next().map(|game| GameMetadata {
        igdb_id: game.id,
        name: game.name,
        summary: game.summary,
        // t_cover_big is the size that looks good in the grid of the library
        // and does not make the page too heavy.
        cover_url: game.cover.map(|cover| cover_url(&cover, "t_cover_big")),
        released_at: game
            .first_release_date
            .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok()),
        genres: game.genres.into_iter().map(|genre| genre.name).collect(),
    }))
}

/// The durations of one batch, indexed by the IGDB record.
///
/// A zero is not a duration: no game is complete in zero seconds, and IGDB
/// writes it where nobody answered. It becomes `None`, so that the interface
/// shows "unknown" and not "0 h". A row with no duration at all is dropped: to
/// keep it would show an empty block with a number of players.
pub fn parse_times_to_beat(body: &str) -> Result<Vec<(i64, TimeToBeat)>> {
    let parsed: Vec<RawTimeToBeat> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("unreadable answer: {e}")))?;

    Ok(parsed
        .into_iter()
        .map(|raw| {
            let positive = |value: Option<i64>| value.filter(|seconds| *seconds > 0);
            (
                raw.game_id,
                TimeToBeat {
                    hastily: positive(raw.hastily),
                    normally: positive(raw.normally),
                    completely: positive(raw.completely),
                    submissions: raw.count.unwrap_or(0).max(0),
                },
            )
        })
        .filter(|(_, time)| !time.is_empty())
        .collect())
}

fn year_of(timestamp: i64) -> Option<i32> {
    OffsetDateTime::from_unix_timestamp(timestamp)
        .ok()
        .map(|date| date.year())
}
