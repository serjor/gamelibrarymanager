//! Cliente de IsThereAnyDeal.
//!
//! Dos preguntas, y en este orden:
//!
//! 1. Qué juego de ITAD es este, que se resuelve con el appid de Steam cuando lo
//!    hay y por título cuando no. Igual que en IGDB, el identificador cruzado es
//!    exacto y el título es una apuesta.
//! 2. Cuánto cuesta hoy y cuánto ha llegado a costar. Va por lotes: una petición
//!    por juego convertiría una lista de deseados en cien peticiones.
//!
//! ## Vigencia de los endpoints (comprobado el 2026-08-15)
//!
//! ITAD sí tiene API pública y documentada, así que aquí no hay la incertidumbre
//! de GOG o Epic. Aun así se comprobó, porque la v2 movió endpoints de sitio:
//!
//! - `GET /games/lookup/v1` **vive**: sin clave responde
//!   `403 {"reason_phrase":"Missing api key"}`, es decir, acepta la ruta y solo
//!   rechaza la credencial.
//! - `POST /games/prices/v3` **vive**: con `GET` responde 405, con `POST` y sin
//!   clave responde 403. Devuelve en la misma respuesta las ofertas de cada
//!   tienda **y** `historyLow`, así que no hace falta llamar además a
//!   `/games/historylow/v1`.
//! - El límite publicado es de 1000 peticiones cada cinco minutos para una
//!   cuenta con correo verificado, y un 429 llega con `Retry-After`.
//!
//! La clave puede ir como parámetro `key` o como cabecera `ITAD-API-Key`. Se usa
//! la cabecera: una clave en la URL acaba en cualquier registro por el que pase
//! la petición.

mod parse;

use std::time::Duration;

use domain::GamePrices;
use serde::{Deserialize, Serialize};

use crate::rate_limit::RateLimiter;
use crate::{MetadataError, Result};

const API: &str = "https://api.isthereanydeal.com";

/// Cuántos juegos caben en una petición de precios. Lo fija ITAD.
const MAX_BATCH: usize = 200;

/// Lo que el usuario aporta para consultar precios.
///
/// El país no es un adorno: ITAD devuelve las tiendas y la moneda de ese
/// mercado, y pedir precios sin decir dónde vives da el precio de otro sitio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItadCredentials {
    pub key: String,
    /// Código ISO de dos letras.
    pub country: String,
}

/// Un juego tal y como lo identifica ITAD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItadGame {
    /// El UUID que aceptan las consultas por lotes.
    pub id: String,
    /// El nombre con el que se construye la dirección de su página.
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

    /// Redirige las llamadas a otro host. Existe para los tests.
    pub fn with_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    /// El juego exacto a partir del appid de Steam.
    pub async fn lookup_by_steam_app_id(
        &self,
        credentials: &ItadCredentials,
        app_id: &str,
    ) -> Result<Option<ItadGame>> {
        self.lookup(credentials, &[("appid", app_id)]).await
    }

    /// El juego que ITAD cree que es este título.
    ///
    /// Para las copias que no vienen de Steam, que no traen ningún
    /// identificador cruzado. ITAD resuelve el título por su cuenta y devuelve
    /// uno o ninguno: aquí no hay una lista de candidatos que puntuar.
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

    /// Los precios de varios juegos, en tandas del tamaño que admite ITAD.
    ///
    /// Devuelve solo los juegos de los que sabe algo: uno que no vende nadie no
    /// aparece en la respuesta, y eso no es un error.
    pub async fn prices(
        &self,
        credentials: &ItadCredentials,
        itad_ids: &[String],
    ) -> Result<Vec<GamePrices>> {
        let mut todos = Vec::with_capacity(itad_ids.len());

        for lote in itad_ids.chunks(MAX_BATCH) {
            self.limiter.acquire().await;

            let response = self
                .http
                .post(format!("{}/games/prices/v3", self.api_base))
                .header("ITAD-API-Key", &credentials.key)
                .query(&[("country", credentials.country.as_str())])
                .json(&lote)
                .send()
                .await
                .map_err(|e| MetadataError::Transport(e.to_string()))?;

            todos.extend(parse::parse_prices(&self.body(response).await?)?);
        }

        Ok(todos)
    }

    /// El cuerpo de una respuesta, o el error que le corresponde.
    ///
    /// ITAD contesta 403 —no 401— cuando falta la clave o no vale.
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
