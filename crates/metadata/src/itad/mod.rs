//! The IsThereAnyDeal client.
//!
//! Two questions, and in this order:
//!
//! 1. Which ITAD game this is. The Steam appid resolves it when there is one,
//!    and the title resolves it when there is not. As with IGDB, the identifier
//!    in common is exact and the title is a guess.
//! 2. What it costs today and the lowest price that it has had. This goes in
//!    batches: one request for each game would turn a wishlist into one hundred
//!    requests.
//!
//! ## The endpoints (examined on 2026-08-15)
//!
//! ITAD has a public API with documentation, thus there is not the uncertainty
//! of GOG or Epic here. The endpoints were still examined, because v2 moved some
//! of them:
//!
//! - `GET /games/lookup/v1` is **alive**: with no key it answers
//!   `403 {"reason_phrase":"Missing api key"}`, which shows that it accepts the
//!   path and refuses only the credential.
//! - `POST /games/prices/v3` is **alive**: with `GET` it answers 405, and with
//!   `POST` and no key it answers 403. In the same answer it gives the offers of
//!   each store **and** `historyLow`, thus a second call to
//!   `/games/historylow/v1` is unnecessary.
//! - The published limit is 1000 requests each five minutes for an account with
//!   a verified mail address, and a 429 comes with `Retry-After`.
//!
//! The key can go as the `key` parameter or as the `ITAD-API-Key` header. This
//! code uses the header: a key in the URL goes into each log through which the
//! request passes.

mod parse;

use std::time::Duration;

use domain::GamePrices;
use serde::{Deserialize, Serialize};

use crate::rate_limit::RateLimiter;
use crate::{MetadataError, Result};

const API: &str = "https://api.isthereanydeal.com";

/// How many games go in one price request. ITAD sets this limit.
const MAX_BATCH: usize = 200;

/// What the user supplies to ask for prices.
///
/// The country is not decoration: ITAD gives back the stores and the currency of
/// that market, and a price request that does not say where you live gives the
/// price of a different place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItadCredentials {
    pub key: String,
    /// The ISO code of two letters.
    pub country: String,
}

/// A game as ITAD identifies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItadGame {
    /// The UUID that the batch queries accept.
    pub id: String,
    /// The name with which you build the address of its page.
    pub slug: String,
    pub title: String,
}

pub struct ItadClient {
    http: reqwest::Client,
    api_base: String,
    limiter: RateLimiter,
}

impl ItadClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            api_base: API.to_owned(),
            limiter: RateLimiter::new(1000, Duration::from_secs(300)),
        }
    }

    /// Sends the calls to a different host. It exists for the tests.
    pub fn with_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    /// The exact game, from the Steam appid.
    pub async fn lookup_by_steam_app_id(
        &self,
        credentials: &ItadCredentials,
        app_id: &str,
    ) -> Result<Option<ItadGame>> {
        self.lookup(credentials, &[("appid", app_id)]).await
    }

    /// The game that ITAD thinks this title is.
    ///
    /// This is for the copies that do not come from Steam, which carry no
    /// identifier in common. ITAD resolves the title alone and gives back one
    /// game or none: there is no list of candidates to score here.
    pub async fn lookup_by_title(
        &self,
        credentials: &ItadCredentials,
        title: &str,
    ) -> Result<Option<ItadGame>> {
        self.lookup(credentials, &[("title", title)]).await
    }

    async fn lookup(
        &self,
        credentials: &ItadCredentials,
        query: &[(&str, &str)],
    ) -> Result<Option<ItadGame>> {
        self.limiter.acquire().await;

        let response = self
            .http
            .get(format!("{}/games/lookup/v1", self.api_base))
            .header("ITAD-API-Key", &credentials.key)
            .query(query)
            .send()
            .await
            .map_err(|e| MetadataError::Transport(e.to_string()))?;

        parse::parse_lookup(&self.body(response).await?)
    }

    /// The prices of more than one game, in batches of the size that ITAD
    /// accepts.
    ///
    /// It gives back only the games that it knows: a game that nobody sells does
    /// not appear in the answer, and that is not an error.
    pub async fn prices(
        &self,
        credentials: &ItadCredentials,
        itad_ids: &[String],
    ) -> Result<Vec<GamePrices>> {
        let mut all = Vec::with_capacity(itad_ids.len());

        for batch in itad_ids.chunks(MAX_BATCH) {
            self.limiter.acquire().await;

            let response = self
                .http
                .post(format!("{}/games/prices/v3", self.api_base))
                .header("ITAD-API-Key", &credentials.key)
                .query(&[("country", credentials.country.as_str())])
                .json(&batch)
                .send()
                .await
                .map_err(|e| MetadataError::Transport(e.to_string()))?;

            all.extend(parse::parse_prices(&self.body(response).await?)?);
        }

        Ok(all)
    }

    /// The body of an answer, or the applicable error.
    ///
    /// ITAD answers 403 — not 401 — when the key is absent or is not valid.
    async fn body(&self, response: reqwest::Response) -> Result<String> {
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
