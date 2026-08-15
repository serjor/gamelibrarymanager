//! The GOG connector.
//!
//! GOG has no public API and does not let you register a client of your own: the
//! only client that its authorisation server accepts is the client of GOG
//! Galaxy. Thus the `client_id`/`client_secret` pair does not go inside the
//! binary. The user supplies it when they connect the account, as with the Steam
//! key, and it lives where the other credentials live: in the store of secrets.
//!
//! The GOG password never comes through here. The user identifies themselves on
//! the real GOG page in a webview, and the only data that this code sees is the
//! `code` of the redirect.
//!
//! ## The endpoints (examined on 2026-08-14)
//!
//! The plan recorded endpoints from a 2018 dump and one half of them is no
//! longer applicable:
//!
//! - `auth.gog.com/auth` and `auth.gog.com/token` **continue to operate**. The
//!   token endpoint answers `invalid_grant` to a code that does not exist, which
//!   shows that it accepts the client and refuses only the code.
//! - `embed.gog.com/user/data/games` and
//!   `embed.gog.com/account/getFilteredProducts` are **dead**: they answer 302
//!   to the login screen. Heroic replaced them in its PR #5718 (June 2026) and
//!   keeps no reference to `embed.gog.com` in its library code.
//! - Today the library is read from
//!   `galaxy-library.gog.com/users/{id}/releases`, in pages through
//!   `page_token`.

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

/// The address to which GOG redirects at the end of the login. It is not a page
/// of this project and it does not need a server behind it: you only must
/// recognise it to take the `code` from its query string.
pub const REDIRECT_URI: &str = "https://embed.gog.com/on_login_success?origin=client";

/// The margin with which a token that is still live counts as expired. A token
/// that expires during a request is a 401 that nobody asked for.
const EXPIRY_MARGIN_SECONDS: i64 = 60;

/// What the connector keeps in the store of secrets. It is opaque to the
/// remainder of the system, which only moves it between the store and this
/// connector.
///
/// It contains the client credentials because the refresh needs them: without
/// them you would have to ask the user for them again each time that the token
/// expires.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GogCredential {
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    user_id: String,
    /// Unix time, in seconds.
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

    /// Sends the calls to a different host. It exists for the tests: the suite
    /// never calls the real API.
    pub fn with_bases(mut self, base: &str) -> Self {
        self.auth_base = base.to_owned();
        self.galaxy_base = base.to_owned();
        self.api_base = base.to_owned();
        self.users_base = base.to_owned();
        self
    }

    /// The address of the GOG login form, which is the only page that opens in
    /// the webview.
    pub fn authorize_url(auth_base: &str, client_id: &str) -> String {
        format!(
            "{auth_base}/auth?client_id={client_id}\
             &redirect_uri={redirect}\
             &response_type=code&layout=client2",
            redirect = urlencode(REDIRECT_URI),
        )
    }

    /// Takes the `code` from the final redirect. It gives back `None` while the
    /// user continues through the login, which is most of the calls.
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
            // GOG answers 400 both to an expired code and to an incorrect
            // client pair. In the two conditions the user must connect the
            // account again, thus the two become the same error.
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

    /// The titles in batches. Without this, the GOG library would come as a
    /// list of numbers, and the matching by title would have no data.
    async fn products(
        &self,
        ids: &[String],
        access_token: &str,
    ) -> std::collections::HashMap<String, parse::ProductInfo> {
        let mut products = std::collections::HashMap::new();
        for chunk in ids.chunks(50) {
            let url = format!("{}/products?ids={}", self.api_base, chunk.join(","));
            // A batch that fails leaves its games with a temporary name, but it
            // does not stop all of the synchronisation.
            if let Ok(body) = self.get(&url, access_token).await {
                products.extend(parse_products(&body));
            }
        }
        products
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

                // The name of the account is decoration: if it fails, the
                // account still connects and shows its identifier.
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

            // The refresh lives here. `sync` asks for the session again at each
            // pass, thus it is sufficient to examine the expiry when the session
            // is built again: if the token is still live, it uses no request.
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
                "GOG does not use an API key".to_owned(),
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
        let products = self.products(&ids, &credential.access_token).await;
        Ok(parse::to_entries(&releases, &products, account_id))
    }

    /// GOG does not give the wishlist to a Galaxy token: the only method is
    /// `embed.gog.com/user/wishlist.json`, which uses the session cookie of the
    /// browser and answers 403 to a `Bearer`. An empty list is preferable to a
    /// scrape made with the web session of the user.
    async fn wishlist(
        &self,
        _session: &StoreSession,
        _account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError> {
        Ok(Vec::new())
    }
}

/// The minimum escape to put one URL inside another. It is used only with
/// constants of this project and with GOG page tokens, thus a complete
/// dependency is unnecessary for this.
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
