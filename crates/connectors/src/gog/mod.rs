//! Conector de GOG.
//!
//! GOG no tiene API pública ni permite registrar un cliente propio: el único
//! cliente que su servidor de autorización reconoce es el de GOG Galaxy. Por
//! eso el par `client_id`/`client_secret` no viaja dentro del binario sino que
//! lo aporta el usuario al conectar la cuenta, igual que la clave de Steam, y
//! vive donde viven las demás credenciales: en el almacén de secretos.
//!
//! La contraseña de GOG no pasa por aquí en ningún momento. El usuario se
//! identifica en la página real de GOG dentro de un webview y lo único que este
//! código llega a ver es el `code` de la redirección.
//!
//! ## Vigencia de los endpoints (comprobado el 2026-08-14)
//!
//! El plan documentaba endpoints de un volcado de 2018 y la mitad ya no vale:
//!
//! - `auth.gog.com/auth` y `auth.gog.com/token` **siguen bien**. El token
//!   responde `invalid_grant` a un código inventado, es decir, acepta el
//!   cliente y solo rechaza el código.
//! - `embed.gog.com/user/data/games` y `embed.gog.com/account/getFilteredProducts`
//!   **están muertos**: responden 302 a la pantalla de login. Heroic los
//!   sustituyó en su PR #5718 (junio de 2026) y ya no le queda ni una
//!   referencia a `embed.gog.com` en su código de biblioteca.
//! - La biblioteca se lee hoy de `galaxy-library.gog.com/users/{id}/releases`,
//!   paginada por `page_token`.

mod parse;

use async_trait::async_trait;
use domain::{
    AuthContext, ClientCredentials, ConnectorError, StoreAccountId, StoreConnector, StoreEntry,
    StoreId, StoreSession,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use parse::{parse_products, parse_releases_page, parse_username};

const DEFAULT_AUTH: &str = "https://auth.gog.com";
const DEFAULT_GALAXY: &str = "https://galaxy-library.gog.com";
const DEFAULT_API: &str = "https://api.gog.com";
const DEFAULT_USERS: &str = "https://users.gog.com";

/// A donde redirige GOG al terminar el login. No es una página nuestra ni hace
/// falta que exista un servidor detrás: solo hay que reconocerla para sacar el
/// `code` de su cadena de consulta.
pub const REDIRECT_URI: &str = "https://embed.gog.com/on_login_success?origin=client";

/// Margen con el que se considera caducado un token todavía vivo. Un token que
/// expira en pleno vuelo es un 401 que nadie ha pedido.
const EXPIRY_MARGIN_SECONDS: i64 = 60;

/// Lo que el conector guarda en el almacén de secretos. Opaco para el resto del
/// sistema, que solo lo mueve entre el almacén y este conector.
///
/// Lleva dentro las credenciales de cliente porque el refresco las necesita: sin
/// ellas habría que volver a pedírselas al usuario cada vez que caduca el token.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GogCredential {
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    user_id: String,
    /// Unix, en segundos.
    expires_at: i64,
}

impl GogCredential {
    fn is_valid(&self, now: OffsetDateTime) -> bool {
        self.expires_at - EXPIRY_MARGIN_SECONDS > now.unix_timestamp()
    }
}

pub struct GogConnector {
    http: reqwest::Client,
    auth_base: String,
    galaxy_base: String,
    api_base: String,
    users_base: String,
}

impl GogConnector {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            auth_base: DEFAULT_AUTH.to_owned(),
            galaxy_base: DEFAULT_GALAXY.to_owned(),
            api_base: DEFAULT_API.to_owned(),
            users_base: DEFAULT_USERS.to_owned(),
        }
    }

    /// Redirige las llamadas a otro host. Existe para los tests: nunca se llama
    /// a la API real desde la suite.
    pub fn with_bases(mut self, base: &str) -> Self {
        self.auth_base = base.to_owned();
        self.galaxy_base = base.to_owned();
        self.api_base = base.to_owned();
        self.users_base = base.to_owned();
        self
    }

    /// La dirección del formulario de login de GOG, que es lo único que se abre
    /// en el webview.
    pub fn authorize_url(auth_base: &str, client_id: &str) -> String {
        format!(
            "{auth_base}/auth?client_id={client_id}\
             &redirect_uri={redirect}\
             &response_type=code&layout=client2",
            redirect = urlencode(REDIRECT_URI),
        )
    }

    /// Saca el `code` de la redirección final. Devuelve `None` mientras el
    /// usuario siga navegando por el login, que es la mayoría de las veces que
    /// se llama.
    pub fn code_from_redirect(url: &str) -> Option<String> {
        let (base, query) = url.split_once('?')?;
        if !base.starts_with("https://embed.gog.com/on_login_success") {
            return None;
        }
        query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| *key == "code")
            .map(|(_, value)| value.to_owned())
            .filter(|code| !code.is_empty())
    }

    async fn token(
        &self,
        client: &ClientCredentials,
        grant: &[(&str, &str)],
    ) -> Result<GogCredential, ConnectorError> {
        let mut query: Vec<(&str, &str)> = vec![
            ("client_id", client.client_id.as_str()),
            ("client_secret", client.client_secret.as_str()),
        ];
        query.extend_from_slice(grant);

        let response = self
            .http
            .get(format!("{}/token", self.auth_base))
            .query(&query)
            .send()
            .await
            .map_err(|e| ConnectorError::Transport(e.to_string()))?;

        match response.status().as_u16() {
            200 => {}
            // GOG contesta 400 tanto al código caducado como al par de cliente
            // equivocado. En ambos casos lo que hay que hacer es volver a
            // conectar la cuenta, así que se traduce igual.
            400 | 401 | 403 => return Err(ConnectorError::Unauthorized),
            429 => return Err(ConnectorError::RateLimited),
            other => return Err(ConnectorError::Unexpected(format!("HTTP {other}"))),
        }

        let body = response
            .text()
            .await
            .map_err(|e| ConnectorError::Transport(e.to_string()))?;
        let token: parse::TokenResponse =
            serde_json::from_str(&body).map_err(|e| ConnectorError::Unexpected(e.to_string()))?;

        Ok(GogCredential {
            client_id: client.client_id.clone(),
            client_secret: client.client_secret.clone(),
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            user_id: token.user_id,
            expires_at: OffsetDateTime::now_utc().unix_timestamp() + token.expires_in,
        })
    }

    async fn get(&self, url: &str, access_token: &str) -> Result<String, ConnectorError> {
        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
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

    fn credential(session: &StoreSession) -> Result<GogCredential, ConnectorError> {
        serde_json::from_str(&session.credential).map_err(|_| ConnectorError::Unauthorized)
    }

    fn session_from(credential: &GogCredential, display_name: Option<String>) -> StoreSession {
        StoreSession {
            store: StoreId::Gog,
            account_ref: credential.user_id.clone(),
            display_name,
            credential: serde_json::to_string(credential).unwrap_or_default(),
            expires_at: OffsetDateTime::from_unix_timestamp(credential.expires_at).ok(),
        }
    }

    /// Títulos por lotes. Sin esto la biblioteca de GOG llegaría como una lista
    /// de números, y el emparejamiento por título no tendría con qué trabajar.
    async fn productos(
        &self,
        ids: &[String],
        access_token: &str,
    ) -> std::collections::HashMap<String, parse::ProductInfo> {
        let mut productos = std::collections::HashMap::new();
        for chunk in ids.chunks(50) {
            let url = format!("{}/products?ids={}", self.api_base, chunk.join(","));
            // Un lote que falle deja sus juegos con nombre provisional, pero no
            // tumba la sincronización entera.
            if let Ok(body) = self.get(&url, access_token).await {
                productos.extend(parse_products(&body));
            }
        }
        productos
    }
}

#[async_trait]
impl StoreConnector for GogConnector {
    fn id(&self) -> StoreId {
        StoreId::Gog
    }

    async fn authenticate(&self, ctx: &AuthContext) -> Result<StoreSession, ConnectorError> {
        match ctx {
            AuthContext::AuthCode { code, client } => {
                let credential = self
                    .token(
                        client,
                        &[
                            ("grant_type", "authorization_code"),
                            ("code", code.as_str()),
                            ("redirect_uri", REDIRECT_URI),
                        ],
                    )
                    .await?;

                // El nombre de la cuenta es un adorno: si falla, la cuenta se
                // conecta igual y se muestra por su identificador.
                let display_name = self
                    .get(
                        &format!("{}/users/{}", self.users_base, credential.user_id),
                        &credential.access_token,
                    )
                    .await
                    .ok()
                    .and_then(|body| parse_username(&body));

                Ok(Self::session_from(&credential, display_name))
            }

            // Aquí es donde vive el refresco. `sync` vuelve a pedir la sesión
            // en cada pasada, así que basta con comprobar la caducidad al
            // reconstruirla: si el token sigue vivo no se gasta ni una petición.
            AuthContext::Stored { credential } => {
                let stored: GogCredential =
                    serde_json::from_str(credential).map_err(|_| ConnectorError::Unauthorized)?;

                if stored.is_valid(OffsetDateTime::now_utc()) {
                    return Ok(Self::session_from(&stored, None));
                }

                let client = ClientCredentials {
                    client_id: stored.client_id.clone(),
                    client_secret: stored.client_secret.clone(),
                };
                let renewed = self
                    .token(
                        &client,
                        &[
                            ("grant_type", "refresh_token"),
                            ("refresh_token", stored.refresh_token.as_str()),
                        ],
                    )
                    .await?;

                Ok(Self::session_from(&renewed, None))
            }

            AuthContext::ApiKey { .. } => Err(ConnectorError::Unexpected(
                "GOG no usa clave de API".to_owned(),
            )),
        }
    }

    async fn owned(
        &self,
        session: &StoreSession,
        account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError> {
        let credential = Self::credential(session)?;

        let mut releases = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut url = format!("{}/users/{}/releases", self.galaxy_base, credential.user_id);
            if let Some(token) = &page_token {
                url.push_str(&format!("?page_token={}", urlencode(token)));
            }

            let body = self.get(&url, &credential.access_token).await?;
            let (page, next) = parse_releases_page(&body)?;
            releases.extend(page);

            match next {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        let ids: Vec<String> = releases.iter().map(|r| r.external_id.clone()).collect();
        let productos = self.productos(&ids, &credential.access_token).await;
        Ok(parse::to_entries(&releases, &productos, account_id))
    }

    /// GOG no expone la lista de deseados a un token de Galaxy: la única vía es
    /// `embed.gog.com/user/wishlist.json`, que va por la cookie de sesión del
    /// navegador y responde 403 a un `Bearer`. Devolver una lista vacía es
    /// preferible a inventarse un scraping con la sesión web del usuario.
    async fn wishlist(
        &self,
        _session: &StoreSession,
        _account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError> {
        Ok(Vec::new())
    }
}

/// Escape mínimo para meter una URL dentro de otra. Solo se usa con constantes
/// nuestras y con testigos de paginación de GOG, así que no hace falta traer una
/// dependencia entera para esto.
fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_el_code_de_la_redireccion() {
        let code = GogConnector::code_from_redirect(
            "https://embed.gog.com/on_login_success?origin=client&code=ABC123",
        );
        assert_eq!(code.as_deref(), Some("ABC123"));
    }

    #[test]
    fn ignora_las_paginas_del_propio_login() {
        // Mientras el usuario escribe su contraseña se navega por muchas
        // páginas de GOG: ninguna de ellas puede confundirse con el final.
        assert_eq!(
            GogConnector::code_from_redirect("https://login.gog.com/auth?client_id=1&code=no"),
            None
        );
        assert_eq!(
            GogConnector::code_from_redirect(
                "https://embed.gog.com/on_login_success?origin=client"
            ),
            None
        );
    }

    #[test]
    fn la_url_de_login_lleva_la_redireccion_escapada() {
        let url = GogConnector::authorize_url(DEFAULT_AUTH, "123");
        assert!(url.contains("client_id=123"));
        assert!(
            url.contains(
                "redirect_uri=https%3A%2F%2Fembed.gog.com%2Fon_login_success%3Forigin%3Dclient"
            ),
            "sin escapar, GOG corta la redirección en el primer & y devuelve invalid_request: {url}"
        );
    }
}
