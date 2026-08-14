//! Cliente de IGDB.
//!
//! Dos vías de búsqueda, y el orden importa:
//!
//! 1. `external_games`, que cruza el appid de Steam con la ficha de IGDB. Es
//!    exacto y no admite discusión.
//! 2. Búsqueda por nombre, para GOG y Epic, que no tienen identificador
//!    cruzado. Aquí ya no hay certeza y decide `domain::matching`.

mod parse;
mod rate_limit;

use std::time::Duration;

use domain::Candidate;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{MetadataError, Result};
use rate_limit::RateLimiter;

const API: &str = "https://api.igdb.com/v4";
const TWITCH_TOKEN: &str = "https://id.twitch.tv/oauth2/token";

/// Categoría 1 de `external_games`: Steam. Es el único cruce fiable que
/// publica IGDB para las tiendas que nos interesan.
const EXTERNAL_CATEGORY_STEAM: u8 = 1;

/// Credenciales del propio usuario, sacadas de su aplicación de Twitch.
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
        // Margen de un minuto: un token que caduca en vuelo es un 401 gratuito.
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

    /// Redirige las llamadas a otro host. Existe para los tests.
    pub fn with_bases(mut self, api_base: impl Into<String>, token_url: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self.token_url = token_url.into();
        self
    }

    /// Token de aplicación de Twitch. Caduca en unos 60 días, así que se guarda
    /// y se renueva, no se pide en cada arranque.
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

    /// Ficha exacta a partir del appid de Steam.
    pub async fn by_steam_app_id(
        &self,
        credentials: &IgdbCredentials,
        token: &IgdbToken,
        app_id: &str,
    ) -> Result<Option<i64>> {
        let query = format!(
            "fields game; where category = {EXTERNAL_CATEGORY_STEAM} & uid = \"{app_id}\"; limit 1;"
        );
        let body = self
            .post("external_games", credentials, token, query)
            .await?;
        parse::parse_external_game(&body)
    }

    /// Candidatos por nombre, para las tiendas sin identificador cruzado.
    pub async fn search(
        &self,
        credentials: &IgdbCredentials,
        token: &IgdbToken,
        title: &str,
    ) -> Result<Vec<Candidate>> {
        // Las comillas del título romperían la consulta de IGDB.
        let sanitized = title.replace('"', " ");
        let query = format!(
            "search \"{sanitized}\"; \
             fields id, name, alternative_names.name, first_release_date; \
             limit 10;"
        );
        let body = self.post("games", credentials, token, query).await?;
        parse::parse_candidates(&body)
    }

    /// Ficha completa para pintar la biblioteca.
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
