//! Conector de Steam.
//!
//! Steam es la única de las tres tiendas con una vía oficial, y funciona con la
//! clave de API del propio usuario. Eso importa más de lo que parece: según la
//! documentación de Valve, los perfiles privados no son accesibles «salvo que
//! la clave usada pertenezca al mismo steamid consultado». Es decir, con clave
//! propia el usuario lee su biblioteca privada sin tener que abrir su perfil al
//! mundo, cosa que ninguna aplicación web puede ofrecer.
//!
//! Nunca se pide la contraseña de Steam: los términos de uso de la Web API lo
//! prohíben expresamente.
//!
//! ## Vigencia de los endpoints (comprobado el 2026-08-15)
//!
//! Los tres de la Web API —`GetPlayerSummaries`, `GetOwnedGames` y
//! `GetWishlist`— siguen bien. El cuarto no es de la Web API sino de la tienda,
//! y no está documentado: `store.steampowered.com/api/appdetails` **solo
//! contesta a un appid por petición**, y devuelve `null` a la petición entera en
//! cuanto se le mandan dos. Está detallado en `titles`.

mod parse;

use async_trait::async_trait;
use domain::{
    AuthContext, ConnectorError, StoreAccountId, StoreConnector, StoreEntry, StoreId, StoreSession,
};
use serde::{Deserialize, Serialize};

pub use parse::{parse_owned, parse_wishlist};

const DEFAULT_API: &str = "https://api.steampowered.com";
const DEFAULT_STORE: &str = "https://store.steampowered.com";

/// Lo que el conector guarda en el almacén de secretos. Opaco para el resto del
/// sistema, que solo lo mueve entre el keyring y este conector.
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

    /// Redirige las llamadas a otro host. Existe para los tests: nunca se llama
    /// a la API real desde la suite.
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

    /// Títulos de los deseados. `GetWishlist` solo devuelve appids, así que hay
    /// que preguntar por los nombres aparte. Un fallo aquí no invalida la
    /// sincronización: el título definitivo lo pone IGDB en la fase 4.
    ///
    /// Un appid por petición, y esto no es una elección: `appdetails` acepta
    /// varios en la URL y contesta `null` a la petición **entera** en cuanto son
    /// dos. Comprobado el 2026-08-15:
    ///
    /// ```sh
    /// curl "…/api/appdetails?appids=115800&filters=basic"          # {"115800":{…"name":"Owlboy"…}}
    /// curl "…/api/appdetails?appids=115800,235460&filters=basic"   # null
    /// ```
    ///
    /// Por lotes de veinte, ninguna lista de deseados llegaba a tener un solo
    /// título: todas se quedaban en «Steam 115800» y sin nombre no hay ni ficha
    /// ni búsqueda de precio que valga.
    ///
    /// Sale una petición por juego deseado. La tienda corta sobre las doscientas
    /// cada cinco minutos: una lista muy larga perderá los títulos del final, y
    /// los recuperará en la siguiente sincronización.
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

    /// Valida la clave contra la API antes de darla por buena: es lo que
    /// convierte un error de copiar y pegar en un mensaje inmediato en vez de
    /// en una sincronización vacía y desconcertante.
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
                    "Steam no usa código de autorización".to_owned(),
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
