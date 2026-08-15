//! The IGDB client.
//!
//! Two search methods, and the order is important:
//!
//! 1. `external_games`, which joins the identifier of the store to the IGDB
//!    record. It is exact and there is no doubt. It operates for the three
//!    stores.
//! 2. A search by name, for the entries that do not join. Here there is no
//!    certainty, and `domain::matching` decides.
//!
//! ## The coverage of `external_games` (measured on 2026-08-15)
//!
//! Against a real library of 602 Steam copies, 288 GOG copies and 318 Epic
//! copies:
//!
//! - **Steam**, with the appid: 486 of 500 join, which is 97%.
//! - **GOG**, with the `external_id` of Galaxy: 211 of 288, which is 73%. Of the
//!   77 that fail, 69 are not games of the GOG catalogue — 53 keys of Amazon
//!   Luna and Amazon Prime, 10 sound tracks and extras, 6 prologues and demos.
//!   Of the 219 true games, 211 join, which is **96%**, the same figure as
//!   Steam.
//! - **Epic**, with the identifier of the offer: 78 of 80 namespaces, which is
//!   97%. The connector gets that identifier; this module only asks. Why it is
//!   the offer and not the item is in `connectors::epic`.
//!
//! Thus the note that was here — "Steam is the only reliable join" — was false.
//! It was written in phase 4, when Steam was the only store of the project, and
//! nobody measured it again when GOG and Epic came in.
//!
//! ## `category` is obsolete
//!
//! The documentation marks `category` as obsolete and tells you to use
//! `external_game_source`. The identifiers are the same, thus the change is only
//! the name of the field.

mod parse;

use std::collections::HashMap;
use std::time::Duration;

use domain::Candidate;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::rate_limit::RateLimiter;
use crate::{MetadataError, Result};

const API: &str = "https://api.igdb.com/v4";
const TWITCH_TOKEN: &str = "https://id.twitch.tv/oauth2/token";

/// How many identifiers go in one `external_games` query.
///
/// The maximum `limit` in the IGDB documentation is 500, and the `uid = (…)`
/// filter accepts a batch of that size. With 602 Steam copies the difference is
/// 2 requests and not 602, which at 4 requests each second is two seconds and
/// not two minutes and one half.
const BATCH: usize = 500;

/// The store from which an identifier comes, with the number that
/// `external_game_sources` gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSource {
    Steam = 1,
    Gog = 5,
    Epic = 26,
}

/// The credentials of the user, from their own Twitch application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgdbCredentials {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgdbToken {
    pub access_token: String,
    pub expires_at: i64,
}

impl IgdbToken {
    pub fn is_valid(&self, now: OffsetDateTime) -> bool {
        // A margin of one minute: a token that expires during a request is a
        // 401 that you get for nothing.
        self.expires_at - 60 > now.unix_timestamp()
    }
}

pub struct IgdbClient {
    http: reqwest::Client,
    api_base: String,
    token_url: String,
    limiter: RateLimiter,
}

impl IgdbClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            api_base: API.to_owned(),
            token_url: TWITCH_TOKEN.to_owned(),
            limiter: RateLimiter::new(4, Duration::from_secs(1)),
        }
    }

    /// Sends the calls to a different host. It exists for the tests.
    pub fn with_bases(mut self, api_base: impl Into<String>, token_url: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self.token_url = token_url.into();
        self
    }

    /// The application token of Twitch. It expires in approximately 60 days,
    /// thus it is kept and renewed and not requested at each start.
    pub async fn token(&self, credentials: &IgdbCredentials) -> Result<IgdbToken> {
        let response = self
            .http
            .post(&self.token_url)
            .query(&[
                ("client_id", credentials.client_id.as_str()),
                ("client_secret", credentials.client_secret.as_str()),
                ("grant_type", "client_credentials"),
            ])
            .send()
            .await
            .map_err(|e| MetadataError::Transport(e.to_string()))?;

        if response.status() == 400 || response.status() == 401 || response.status() == 403 {
            return Err(MetadataError::Unauthorized);
        }
        let body = response
            .text()
            .await
            .map_err(|e| MetadataError::Transport(e.to_string()))?;

        parse::parse_token(&body, OffsetDateTime::now_utc())
    }

    /// The exact records, from the identifiers of a store.
    ///
    /// The query goes in batches and not one copy at a time. This is not only
    /// speed: at 4 requests each second, a large library took so long to join
    /// that the user cancelled before the end, and the copies that did not join
    /// fell to the search by title, which is the method with doubt.
    ///
    /// The identifiers that IGDB does not know simply do not appear in the map.
    /// That is not an error: it is usual with third-party keys and with the
    /// extras that the stores sell as if they were games.
    pub async fn by_external_ids(
        &self,
        credentials: &IgdbCredentials,
        token: &IgdbToken,
        source: ExternalSource,
        uids: &[String],
    ) -> Result<HashMap<String, i64>> {
        let source = source as u8;
        let mut joins = HashMap::with_capacity(uids.len());

        for batch in uids.chunks(BATCH) {
            // Quotation marks in an identifier would break the query. No store
            // uses them, but the identifier comes from the network.
            let values = batch
                .iter()
                .map(|uid| format!("\"{}\"", uid.replace('"', "")))
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                "fields uid, game; \
                 where external_game_source = {source} & uid = ({values}); \
                 limit {BATCH};"
            );
            let body = self
                .post("external_games", credentials, token, query)
                .await?;

            for (uid, igdb_id) in parse::parse_external_games(&body)? {
                joins.entry(uid).or_insert(igdb_id);
            }
        }

        Ok(joins)
    }

    /// The candidates by name, for the stores with no identifier in common.
    pub async fn search(
        &self,
        credentials: &IgdbCredentials,
        token: &IgdbToken,
        title: &str,
    ) -> Result<Vec<Candidate>> {
        // Quotation marks in the title would break the IGDB query.
        let sanitized = title.replace('"', " ");
        // The cover is requested here, in the request that was already made:
        // the review queue needs it so that the user can tell equal candidates
        // apart, and a second request for each game would use all of the quota
        // of 4 requests each second.
        let query = format!(
            "search \"{sanitized}\"; \
             fields id, name, slug, alternative_names.name, first_release_date, cover.image_id; \
             limit 10;"
        );
        let body = self.post("games", credentials, token, query).await?;
        parse::parse_candidates(&body)
    }

    /// The complete record, to show the library.
    pub async fn game(
        &self,
        credentials: &IgdbCredentials,
        token: &IgdbToken,
        igdb_id: i64,
    ) -> Result<Option<GameMetadata>> {
        let query = format!(
            "fields id, name, summary, first_release_date, cover.image_id, genres.name; \
             where id = {igdb_id}; limit 1;"
        );
        let body = self.post("games", credentials, token, query).await?;
        parse::parse_game(&body)
    }

    async fn post(
        &self,
        endpoint: &str,
        credentials: &IgdbCredentials,
        token: &IgdbToken,
        query: String,
    ) -> Result<String> {
        self.limiter.acquire().await;

        let response = self
            .http
            .post(format!("{}/{endpoint}", self.api_base))
            .header("Client-ID", &credentials.client_id)
            .header("Authorization", format!("Bearer {}", token.access_token))
            .body(query)
            .send()
            .await
            .map_err(|e| MetadataError::Transport(e.to_string()))?;

        match response.status().as_u16() {
            200 => response
                .text()
                .await
                .map_err(|e| MetadataError::Transport(e.to_string())),
            401 | 403 => Err(MetadataError::Unauthorized),
            429 => Err(MetadataError::RateLimited),
            other => Err(MetadataError::Unexpected(format!("HTTP {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameMetadata {
    pub igdb_id: i64,
    pub name: String,
    pub summary: Option<String>,
    pub cover_url: Option<String>,
    pub released_at: Option<OffsetDateTime>,
    pub genres: Vec<String>,
}
