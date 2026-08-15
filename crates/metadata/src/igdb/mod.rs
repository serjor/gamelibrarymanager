//! Cliente de IGDB.
//!
//! Dos vías de búsqueda, y el orden importa:
//!
//! 1. `external_games`, que cruza el identificador de la tienda con la ficha de
//!    IGDB. Es exacto y no admite discusión. Vale para las tres tiendas.
//! 2. Búsqueda por nombre, para lo que no cruza. Aquí ya no hay certeza y decide
//!    `domain::matching`.
//!
//! ## Cobertura de `external_games` (medida el 2026-08-15)
//!
//! Contra una biblioteca real de 602 copias de Steam, 288 de GOG y 318 de Epic:
//!
//! - **Steam**, con el appid: 486 de 500 cruzan, el 97%.
//! - **GOG**, con el `external_id` de Galaxy: 211 de 288, el 73%. De los 77 que
//!   fallan, 69 no son juegos del catálogo de GOG —53 claves de Amazon Luna y
//!   Amazon Prime, 10 bandas sonoras y extras, 6 prólogos y demos—. Sobre los
//!   219 juegos de verdad cruzan 211, **el 96%**, la misma cifra que Steam.
//! - **Epic**, con el identificador de la oferta: 78 de 80 namespaces, el 97%.
//!   El conector lo consigue; aquí solo se consulta. Por qué es la oferta y no
//!   el item está en `connectors::epic`.
//!
//! Es decir, la nota que vivía aquí —«Steam es el único cruce fiable»— era
//! falsa. Se escribió en la fase 4, cuando Steam era la única tienda del
//! proyecto, y nadie la volvió a medir al llegar GOG y Epic.
//!
//! ## `category` está obsoleto
//!
//! La documentación marca `category` como obsoleto y manda usar
//! `external_game_source`. Los identificadores son los mismos, así que el cambio
//! es de nombre de campo y nada más.

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

/// Cuántos identificadores caben en una consulta de `external_games`.
///
/// El tope de `limit` que documenta IGDB es 500, y el filtro `uid = (…)` admite
/// ese mismo lote. Con 602 copias de Steam la diferencia es 2 peticiones en vez
/// de 602, que a 4 por segundo son dos segundos en vez de dos minutos y medio.
const BATCH: usize = 500;

/// Tienda de la que viene un identificador, tal y como la numera
/// `external_game_sources`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSource {
    Steam = 1,
    Gog = 5,
    Epic = 26,
}

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

    /// Fichas exactas a partir de los identificadores de una tienda.
    ///
    /// Se pregunta por lotes y no copia a copia. No es solo velocidad: a 4
    /// peticiones por segundo, una biblioteca grande tardaba tanto en cruzarse
    /// que el usuario cancelaba antes de llegar al final, y lo que se quedaba
    /// sin cruzar caía en la búsqueda por título, que es la vía dudosa.
    ///
    /// Los identificadores que IGDB no conozca sencillamente no aparecen en el
    /// mapa. No es un error: es lo normal en las claves de terceros y en los
    /// extras que las tiendas venden como si fueran juegos.
    pub async fn by_external_ids(
        &self,
        credentials: &IgdbCredentials,
        token: &IgdbToken,
        source: ExternalSource,
        uids: &[String],
    ) -> Result<HashMap<String, i64>> {
        let source = source as u8;
        let mut cruces = HashMap::with_capacity(uids.len());

        for lote in uids.chunks(BATCH) {
            // Las comillas de un identificador romperían la consulta. Ninguna
            // tienda las usa, pero el identificador llega de la red.
            let valores = lote
                .iter()
                .map(|uid| format!("\"{}\"", uid.replace('"', "")))
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                "fields uid, game; \
                 where external_game_source = {source} & uid = ({valores}); \
                 limit {BATCH};"
            );
            let body = self
                .post("external_games", credentials, token, query)
                .await?;

            for (uid, igdb_id) in parse::parse_external_games(&body)? {
                cruces.entry(uid).or_insert(igdb_id);
            }
        }

        Ok(cruces)
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
        // La portada se pide aquí, en la misma petición que ya se gastaba: la
        // cola de revisión la necesita para que el usuario distinga entre
        // candidatos que empatan, y volver a preguntar por ella juego a juego
        // se comería la cuota de 4 peticiones por segundo.
        let query = format!(
            "search \"{sanitized}\"; \
             fields id, name, slug, alternative_names.name, first_release_date, cover.image_id; \
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
