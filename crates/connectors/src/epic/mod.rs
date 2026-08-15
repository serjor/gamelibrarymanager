//! Epic connector.
//!
//! Epic publishes no API for the library of a user. What exists is the private
//! API of its own launcher, and the living reference for it is legendary
//! (GPLv3), which this module follows. No code was copied: the flow and the
//! endpoints were read there and written again in Rust.
//!
//! Epic does not let a third party register a client either, so the pair
//! `client_id`/`client_secret` is the one of the launcher, published in
//! legendary for years. It does not travel inside the binary: the user hands it
//! over when connecting the account, exactly as with GOG, and it lives in the
//! secret store.
//!
//! The password of Epic never passes through here. The user signs in on the
//! real page of Epic inside a webview.
//!
//! ## Where Epic differs from GOG
//!
//! GOG ends the login with a redirection that carries the `code` in the address,
//! so watching the addresses is enough. **Epic does not redirect with a code**:
//! it answers a JSON document on `id/api/redirect`, and the code is a field of
//! that body. The one who opens the window has to read the page, not the
//! address. That is why [`code_from_body`] takes a body and not a URL.
//!
//! ## Endpoint validity (checked on 2026-08-15)
//!
//! - `account-public-service-prod03.ol.epicgames.com/account/api/oauth/token`
//!   **is alive**. With the launcher pair it answers `invalid_grant` and
//!   `authorization_code_not_found` to an invented code, that is, it accepts
//!   the client and only rejects the code.
//! - `launcher-public-service-prod06.ol.epicgames.com`,
//!   `catalog-public-service-prod06.ol.epicgames.com` and
//!   `entitlement-public-service-prod08.ol.epicgames.com` answer `401`: alive,
//!   they only ask for credentials.
//! - `www.epicgames.com/id/api/redirect?clientId=…&responseType=code` answers
//!   `200` with `{"authorizationCode":null,…}` without a session, which is the
//!   shape [`code_from_body`] reads.
//! - `www.epicgames.com/id/login` answers `403` to a command line client, with
//!   or without a browser user agent: Cloudflare guards it. It is not a sign of
//!   a dead endpoint, and it is one more reason for the login to happen in a
//!   webview and not in code of this project.
//!
//! ## Why a copy on Epic has no page of the store (measured on 2026-08-15)
//!
//! Steam and GOG hand over the page of each copy, and the review queue uses it
//! to let a person compare against the IGDB card. Epic does not, and the two
//! ways of getting one were both tried against a real account of 318 games:
//!
//! - **The catalogue item does not carry it.** The store *offer* does —it has
//!   `productSlug` and `catalogNs.mappings[].pageSlug`, which is what
//!   `freeGamesPromotions` answers without credentials— but the item that
//!   `catalog/api/…/bulk/items` returns is not the offer. Of the 318 games,
//!   318 got a title and a cover from it, and **zero** got a slug.
//! - **Guessing the slug from the title fails half the time.** It is what
//!   Heroic does as a fallback. Over 25 real titles of that library, cleaned
//!   the way Heroic cleans them, 13 slugs exist and 12 do not.
//!
//! Heroic gets the real slug from `launcher.store.epicgames.com/graphql`, which
//! answers `catalogNs.mappings` for a namespace. That endpoint is behind a bot
//! check: it answered a challenge to this project's own user agent **and** to a
//! plain browser one, and only let through the one that says
//! `EpicGamesLauncher`. Getting past a bot check by claiming to be the client
//! of the store is not something this connector does.
//!
//! So the field stays empty. A link that lands on the wrong page in the very
//! screen that exists for comparing is worse than no link, and it is the same
//! call the GOG connector makes about the wish list.
//!
//! The names of the image types and the rest of the item shape were checked
//! against `store-site-backend-static.ak.epicgames.com/freeGamesPromotions`,
//! and confirmed afterwards against the real library.

mod parse;

use std::collections::HashMap;

use async_trait::async_trait;
use domain::{
    AuthContext, ClientCredentials, ConnectorError, StoreAccountId, StoreConnector, StoreEntry,
    StoreId, StoreSession,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use parse::{ItemInfo, parse_assets, parse_items};

const DEFAULT_OAUTH: &str = "https://account-public-service-prod03.ol.epicgames.com";
const DEFAULT_LAUNCHER: &str = "https://launcher-public-service-prod06.ol.epicgames.com";
const DEFAULT_CATALOG: &str = "https://catalog-public-service-prod06.ol.epicgames.com";

/// Sign in page of Epic. It is the only address the webview opens.
const LOGIN_PAGE: &str = "https://www.epicgames.com/id/login";

/// Page that mints the authorisation code once there is a session. It answers
/// JSON, and the user lands on it because the login redirects there.
pub const AUTHORIZATION_PAGE: &str = "https://www.epicgames.com/id/api/redirect";

/// Platform whose assets are read.
///
/// It is not a limitation of what the user sees: Epic gives every game a
/// Windows asset, and legendary reads this same list on the three desktops. The
/// native builds for Mac are a subset, so asking for them would hide games.
const PLATFORM: &str = "Windows";

/// Label of the live builds. The launcher uses others for its test channels.
const LABEL: &str = "Live";

/// Margin with which a token that is still alive counts as expired. A token
/// that expires mid flight is a 401 nobody asked for.
const EXPIRY_MARGIN_SECONDS: i64 = 60;

/// What the connector keeps in the secret store. Opaque for the rest of the
/// system, which only moves it between the store and this connector.
///
/// It carries the client credentials because the refresh needs them: without
/// them the user would have to hand them over again on every expiry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EpicCredential {
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    account_id: String,
    /// Unix, in seconds.
    expires_at: i64,
}

impl EpicCredential {
    fn is_valid(&self, now: OffsetDateTime) -> bool {
        self.expires_at - EXPIRY_MARGIN_SECONDS > now.unix_timestamp()
    }
}

pub struct EpicConnector {
    http: reqwest::Client,
    oauth_base: String,
    launcher_base: String,
    catalog_base: String,
}

impl EpicConnector {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            oauth_base: DEFAULT_OAUTH.to_owned(),
            launcher_base: DEFAULT_LAUNCHER.to_owned(),
            catalog_base: DEFAULT_CATALOG.to_owned(),
        }
    }

    /// Points the calls at another host. It exists for the tests: the suite
    /// never calls the real API.
    pub fn with_bases(mut self, base: &str) -> Self {
        self.oauth_base = base.to_owned();
        self.launcher_base = base.to_owned();
        self.catalog_base = base.to_owned();
        self
    }

    /// The address of the Epic sign in form, which is the only one the webview
    /// opens.
    ///
    /// After the user signs in, Epic sends the window to [`AUTHORIZATION_PAGE`],
    /// and there the code is waiting in the body.
    pub fn authorize_url(client_id: &str) -> String {
        let destination = format!("{AUTHORIZATION_PAGE}?clientId={client_id}&responseType=code");
        format!("{LOGIN_PAGE}?redirectUrl={}", urlencode(&destination))
    }

    /// Whether an address is the page that carries the code.
    ///
    /// The window navigates through many pages of Epic while the user signs in,
    /// and the code must only be looked for in this one: reading the body of
    /// the page where the password is typed is exactly what this project does
    /// not do.
    pub fn is_authorization_page(url: &str) -> bool {
        url.starts_with(AUTHORIZATION_PAGE)
    }

    /// Takes the code out of the body of [`AUTHORIZATION_PAGE`].
    ///
    /// Answers `None` while there is no session, which is what the page itself
    /// says with `"authorizationCode": null`.
    pub fn code_from_body(body: &str) -> Option<String> {
        serde_json::from_str::<parse::AuthorizationResponse>(body)
            .ok()?
            .authorization_code
            .filter(|code| !code.is_empty())
    }

    /// Exchanges a grant for a credential, and hands back the account name that
    /// travels with it.
    ///
    /// The name is not part of the credential: it is an ornament for the
    /// interface, the database already keeps it, and the secret store is no
    /// place for anything that does not open a door.
    async fn token(
        &self,
        client: &ClientCredentials,
        grant: &[(&str, &str)],
    ) -> Result<(EpicCredential, Option<String>), ConnectorError> {
        // `token_type=eg1` is what legendary asks for: it is the token the
        // launcher services accept. Without it the answer is a token for the
        // account services alone and the library never arrives.
        let mut form: Vec<(&str, &str)> = vec![("token_type", "eg1")];
        form.extend_from_slice(grant);

        let response = self
            .http
            .post(format!("{}/account/api/oauth/token", self.oauth_base))
            // The client pair travels in the `Authorization` header, not in the
            // body: it is the only shape this endpoint accepts.
            .basic_auth(&client.client_id, Some(&client.client_secret))
            .form(&form)
            .send()
            .await
            .map_err(|e| ConnectorError::Transport(e.to_string()))?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| ConnectorError::Transport(e.to_string()))?;

        match status {
            200 => {}
            400 | 401 | 403 => return Err(auth_error(&body)),
            429 => return Err(ConnectorError::RateLimited),
            other => return Err(ConnectorError::Unexpected(format!("HTTP {other}"))),
        }

        let token: parse::TokenResponse =
            serde_json::from_str(&body).map_err(|e| ConnectorError::Unexpected(e.to_string()))?;

        Ok((
            EpicCredential {
                client_id: client.client_id.clone(),
                client_secret: client.client_secret.clone(),
                access_token: token.access_token,
                refresh_token: token.refresh_token,
                account_id: token.account_id,
                expires_at: OffsetDateTime::now_utc().unix_timestamp() + token.expires_in,
            },
            token.display_name,
        ))
    }

    async fn get(
        &self,
        url: &str,
        query: &[(&str, &str)],
        access_token: &str,
    ) -> Result<String, ConnectorError> {
        let response = self
            .http
            .get(url)
            .query(query)
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

    fn credential(session: &StoreSession) -> Result<EpicCredential, ConnectorError> {
        serde_json::from_str(&session.credential).map_err(|_| ConnectorError::Unauthorized)
    }

    fn session_from(credential: &EpicCredential, display_name: Option<String>) -> StoreSession {
        StoreSession {
            store: StoreId::Epic,
            account_ref: credential.account_id.clone(),
            display_name,
            credential: serde_json::to_string(credential).unwrap_or_default(),
            expires_at: OffsetDateTime::from_unix_timestamp(credential.expires_at).ok(),
        }
    }

    /// Names, covers and pages of what the account owns.
    ///
    /// One request per game, which is what legendary does. The endpoint is
    /// called `bulk/items` and its answer is a map, so asking for several
    /// identifiers at once looks possible, but each game lives in its own
    /// namespace and the address carries the namespace: grouping would save
    /// almost nothing and it is not what the living reference does. A batch
    /// that fails leaves its game with a provisional name and does not bring
    /// down the whole synchronisation.
    async fn items(
        &self,
        assets: &[parse::Asset],
        access_token: &str,
    ) -> HashMap<String, ItemInfo> {
        let mut items = HashMap::with_capacity(assets.len());

        for asset in assets {
            let url = format!(
                "{}/catalog/api/shared/namespace/{}/bulk/items",
                self.catalog_base, asset.namespace
            );
            // English and the United States, and it is not carelessness: the
            // title is what the matching hands to IGDB, and IGDB names its
            // entries in English. Asking Epic for Spanish would turn every
            // title into a translation the matching then fails to find.
            let query = [
                ("id", asset.catalog_item_id.as_str()),
                ("includeDLCDetails", "true"),
                ("includeMainGameDetails", "true"),
                ("country", "US"),
                ("locale", "en"),
            ];

            if let Ok(body) = self.get(&url, &query, access_token).await {
                items.extend(parse_items(&body));
            }
        }

        items
    }
}

#[async_trait]
impl StoreConnector for EpicConnector {
    fn id(&self) -> StoreId {
        StoreId::Epic
    }

    async fn authenticate(&self, ctx: &AuthContext) -> Result<StoreSession, ConnectorError> {
        match ctx {
            // Epic hands the account name back in the token answer itself, so
            // unlike GOG there is no second request to make for it.
            AuthContext::AuthCode { code, client } => {
                let (credential, display_name) = self
                    .token(
                        client,
                        &[
                            ("grant_type", "authorization_code"),
                            ("code", code.as_str()),
                        ],
                    )
                    .await?;

                Ok(Self::session_from(&credential, display_name))
            }

            // The refresh lives here. `sync` asks for the session on every
            // pass, so checking the expiry while rebuilding it is enough: if
            // the token is still alive it does not spend a single request.
            AuthContext::Stored { credential } => {
                let stored: EpicCredential =
                    serde_json::from_str(credential).map_err(|_| ConnectorError::Unauthorized)?;

                if stored.is_valid(OffsetDateTime::now_utc()) {
                    return Ok(Self::session_from(&stored, None));
                }

                let client = ClientCredentials {
                    client_id: stored.client_id.clone(),
                    client_secret: stored.client_secret.clone(),
                };
                let (renewed, _) = self
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
                "Epic no usa clave de API".to_owned(),
            )),
        }
    }

    async fn owned(
        &self,
        session: &StoreSession,
        account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError> {
        let credential = Self::credential(session)?;

        let url = format!(
            "{}/launcher/api/public/assets/{PLATFORM}",
            self.launcher_base
        );
        let body = self
            .get(&url, &[("label", LABEL)], &credential.access_token)
            .await?;
        let assets = parse_assets(&body)?;

        let items = self.items(&assets, &credential.access_token).await;
        Ok(parse::to_entries(&assets, &items, account_id))
    }

    /// Epic keeps the wish list in the GraphQL of its store, which answers to
    /// the session of the web and not to a launcher token. An empty list is
    /// honest; scraping with the browser session of the user is not an option
    /// this project takes.
    async fn wishlist(
        &self,
        _session: &StoreSession,
        _account_id: StoreAccountId,
    ) -> Result<Vec<StoreEntry>, ConnectorError> {
        Ok(Vec::new())
    }
}

/// Translates a rejected token into something the user can act on.
///
/// Epic answers 400 both to a spent code and to a wrong client pair, and in
/// both cases the way out is to connect the account again. There is a third
/// case that is not: when the account itself has something pending —new terms,
/// an unverified mail— no application can authorise until the user opens the
/// page Epic hands over, and saying only "invalid credentials" would send them
/// to retry the login forever.
fn auth_error(body: &str) -> ConnectorError {
    let error: parse::ErrorResponse = serde_json::from_str(body).unwrap_or_default();

    match error.continuation_url {
        Some(url) => {
            let reason = error
                .error_message
                .unwrap_or_else(|| "Epic pide una acción en tu cuenta".to_owned());
            ConnectorError::Unexpected(format!(
                "{reason}. Abre esta página de Epic y vuelve a conectar la cuenta: {url}"
            ))
        }
        None => ConnectorError::Unauthorized,
    }
}

/// Minimal escape to put one URL inside another. It is only used with our own
/// constants and with the client identifier, so there is no need to pull in a
/// whole dependency for this.
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
    fn reads_the_code_from_the_body_and_not_from_the_address() {
        // This is the difference with GOG: Epic does not redirect with the code
        // in the address, it answers a JSON document.
        let body = r#"{"redirectUrl":"https://localhost/launcher/authorized",
                       "authorizationCode":"ABC123","exchangeCode":null,"sid":null}"#;
        assert_eq!(
            EpicConnector::code_from_body(body).as_deref(),
            Some("ABC123")
        );
    }

    #[test]
    fn without_a_session_the_page_answers_with_no_code() {
        // Literal answer of the page when nobody has signed in yet. Taking the
        // null for a code would send an empty string to the token endpoint.
        let body = r#"{"warning":"Do not share this code with any 3rd party service.",
                       "redirectUrl":"https://localhost/launcher/authorized",
                       "authorizationCode":null,"exchangeCode":null,"sid":null}"#;
        assert_eq!(EpicConnector::code_from_body(body), None);
        assert_eq!(EpicConnector::code_from_body("no es json"), None);
    }

    #[test]
    fn only_the_authorization_page_is_read() {
        assert!(EpicConnector::is_authorization_page(
            "https://www.epicgames.com/id/api/redirect?clientId=1&responseType=code"
        ));
        // The page where the password is typed is not read, and neither is a
        // host that only starts the same way.
        assert!(!EpicConnector::is_authorization_page(
            "https://www.epicgames.com/id/login"
        ));
        assert!(!EpicConnector::is_authorization_page(
            "https://www.epicgames.com.evil.test/id/api/redirect"
        ));
    }

    #[test]
    fn the_login_address_carries_the_destination_escaped() {
        let url = EpicConnector::authorize_url("34a02cf8f4414e29b15921876da36f9a");
        assert!(
            url.contains(
                "redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode"
            ),
            "unescaped, Epic cuts the destination at the first & and never mints a code: {url}"
        );
    }

    #[test]
    fn a_pending_account_says_what_to_do() {
        let error = auth_error(
            r#"{"errorCode":"errors.com.epicgames.oauth.corrective_action_required",
                "errorMessage":"Corrective action is required to continue",
                "continuationUrl":"https://www.epicgames.com/id/login/continuation?code=x"}"#,
        );
        assert!(
            error.to_string().contains("continuation"),
            "the message has to carry the page Epic asks the user to open: {error}"
        );
    }

    #[test]
    fn a_spent_code_only_asks_to_connect_again() {
        let error = auth_error(
            r#"{"errorCode":"errors.com.epicgames.account.oauth.authorization_code_not_found",
                "errorMessage":"Sorry the authorization code you supplied was not found."}"#,
        );
        assert!(matches!(error, ConnectorError::Unauthorized));
    }
}
