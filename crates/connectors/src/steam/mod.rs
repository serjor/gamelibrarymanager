//! The Steam connector.
//!
//! Steam is the only one of the three stores with an official method, and it
//! operates with the API key of the user. That is more important than it looks:
//! the documentation of Valve says that private profiles are not accessible
//! "unless the key used belongs to the same steamid being requested". Thus, with
//! a key of their own, the user reads their private library and does not make
//! their profile public. No web application can give that.
//!
//! The Steam password is never requested: the terms of use of the Web API
//! prohibit it clearly.
//!
//! ## The endpoints (examined on 2026-08-15)
//!
//! The three endpoints of the Web API — `GetPlayerSummaries`, `GetOwnedGames`
//! and `GetWishlist` — continue to operate. The fourth one is not from the Web
//! API but from the store, and it has no documentation:
//! `store.steampowered.com/api/appdetails` **answers only one appid for each
//! request**, and gives `null` for all of the request as soon as you send two.
//! `titles` gives the details.

mod parse;

use async_trait::async_trait;
use domain::{
    AuthContext, ConnectorError, StoreAccountId, StoreConnector, StoreEntry, StoreId, StoreSession,
};
use serde::{Deserialize, Serialize};

pub use parse::{parse_owned, parse_wishlist};

const DEFAULT_API: &str = "https://api.steampowered.com";
const DEFAULT_STORE: &str = "https://store.steampowered.com";

/// What the connector keeps in the store of secrets. It is opaque to the
/// remainder of the system, which only moves it between the keyring and this
/// connector.
#[derive(Debug, Serialize, Deserialize)]
struct SteamCredential {
    api_key: String,
}

pub struct SteamConnector {
    http: reqwest::Client,
    api_base: String,
    store_base: String,
}

impl SteamConnector {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            api_base: DEFAULT_API.to_owned(),
            store_base: DEFAULT_STORE.to_owned(),
        }
    }

    /// Sends the calls to a different host. It exists for the tests: the suite
    /// never calls the real API.
    pub fn with_bases(
        mut self,
        api_base: impl Into<String>,
        store_base: impl Into<String>,
    ) -> Self {
        self.api_base = api_base.into();
        self.store_base = store_base.into();
        self
    }

    async fn get(&self, url: &str, query: &[(&str, &str)]) -> Result<String, ConnectorError> {
        let response = self
            .http
            .get(url)
            .query(query)
            .send()
            .await
            .map_err(|e| ConnectorError::Transport(e.to_string()))?;

        match response.status().as_u16() {
            200 => response
                .text()
                .await
                .map_err(|e| ConnectorError::Transport(e.to_string())),
            401 | 403 => Err(ConnectorError::Unauthorized),
            429 => Err(ConnectorError::RateLimited),
            other => Err(ConnectorError::Unexpected(format!("HTTP {other}"))),
        }
    }

    fn credential(session: &StoreSession) -> Result<SteamCredential, ConnectorError> {
        serde_json::from_str(&session.credential).map_err(|_| ConnectorError::Unauthorized)
    }

    /// The titles of the wished-for games. `GetWishlist` gives back only
    /// appids, thus you must ask for the names separately. A failure here does
    /// not make the synchronisation invalid: IGDB sets the final title in phase
    /// 4.
    ///
    /// One appid for each request, and this is not a decision: `appdetails`
    /// accepts more than one in the URL and answers `null` for **all** of the
    /// request as soon as there are two. Examined on 2026-08-15:
    ///
    /// ```sh
    /// curl "…/api/appdetails?appids=115800&filters=basic"          # {"115800":{…"name":"Owlboy"…}}
    /// curl "…/api/appdetails?appids=115800,235460&filters=basic"   # null
    /// ```
    ///
    /// In batches of twenty, no wishlist got even one title: all of them stayed
    /// at "Steam 115800", and with no name there is no usable record and no
    /// usable price search.
    ///
    /// This makes one request for each wished-for game. The store stops at
    /// approximately two hundred requests each five minutes: a very long list
    /// will lose the titles at the end, and will get them at the next
    /// synchronisation.
    async fn titles(&self, app_ids: &[String]) -> std::collections::HashMap<String, String> {
        let url = format!("{}/api/appdetails", self.store_base);
        let mut titles = std::collections::HashMap::new();

        for app_id in app_ids {
            let Ok(body) = self
                .get(&url, &[("appids", app_id.as_str()), ("filters", "basic")])
                .await
            else {
                continue;
            };
            titles.extend(parse::parse_app_details(&body));
        }
        titles
    }
}

#[async_trait]
impl StoreConnector for SteamConnector {
    fn id(&self) -> StoreId {
        StoreId::Steam
    }

    /// Examines the key against the API before it accepts the key: this is what
    /// turns a copy-and-paste error into an immediate message and not into an
    /// empty synchronisation that confuses the user.
    async fn authenticate(&self, ctx: &AuthContext) -> Result<StoreSession, ConnectorError> {
        let (key, steam_id) = match ctx {
            AuthContext::ApiKey { key, account_ref } => (key.clone(), account_ref.clone()),
            AuthContext::Stored { credential } => {
                let parsed: SteamCredential =
                    serde_json::from_str(credential).map_err(|_| ConnectorError::Unauthorized)?;
                return Ok(StoreSession {
                    store: StoreId::Steam,
                    account_ref: String::new(),
                    display_name: None,
                    credential: serde_json::to_string(&parsed).unwrap_or_default(),
                    expires_at: None,
                });
            }
            AuthContext::AuthCode { .. } => {
                return Err(ConnectorError::Unexpected(
                    "Steam does not use an authorisation code".to_owned(),
                ));
            }
        };

        let url = format!("{}/ISteamUser/GetPlayerSummaries/v2/", self.api_base);
        let body = self
            .get(
                &url,
                &[("key", key.as_str()), ("steamids", steam_id.as_str())],
            )
            .await?;
        let display_name = parse::parse_player_name(&body, &steam_id)?;

        Ok(StoreSession {
            store: StoreId::Steam,
            account_ref: steam_id,
            display_name,
            credential: serde_json::to_string(&SteamCredential { api_key: key })
                .map_err(|e| ConnectorError::Unexpected(e.to_string()))?,
            expires_at: None,
        })
    }

    async fn owned(
        &self,
        session: &StoreSession,
        account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError> {
        let credential = Self::credential(session)?;
        let url = format!("{}/IPlayerService/GetOwnedGames/v1/", self.api_base);
        let body = self
            .get(
                &url,
                &[
                    ("key", credential.api_key.as_str()),
                    ("steamid", session.account_ref.as_str()),
                    ("include_appinfo", "1"),
                    ("include_played_free_games", "1"),
                ],
            )
            .await?;

        parse_owned(&body, account_id)
    }

    async fn wishlist(
        &self,
        session: &StoreSession,
        account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError> {
        let credential = Self::credential(session)?;
        let url = format!("{}/IWishlistService/GetWishlist/v1/", self.api_base);
        let body = self
            .get(
                &url,
                &[
                    ("key", credential.api_key.as_str()),
                    ("steamid", session.account_ref.as_str()),
                ],
            )
            .await?;

        let mut entries = parse_wishlist(&body, account_id)?;
        let app_ids: Vec<String> = entries.iter().map(|e| e.store_app_id.clone()).collect();
        let titles = self.titles(&app_ids).await;
        for entry in &mut entries {
            if let Some(title) = titles.get(&entry.store_app_id) {
                entry.title = title.clone();
            }
        }
        Ok(entries)
    }
}
