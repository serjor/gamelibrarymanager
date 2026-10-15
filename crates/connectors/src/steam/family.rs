//! The Steam family: the games that the other members can lend to the user.
//!
//! This part of Steam does not accept the API key of the user. It wants the
//! token of a session signed in to the store, the `webapi_token` that the
//! store gives to its own pages. Thus the key stays the credential of the
//! connector, and the token is a second thing that lives only during one
//! synchronisation: the application never keeps it.
//!
//! ## The endpoints (examined on 2026-10-15)
//!
//! - `IFamilyGroupsService/GetFamilyGroupForUser/v1/` and
//!   `IFamilyGroupsService/GetSharedLibraryApps/v1/` answer **401** with no
//!   credential, with an invented `key=` and with an invalid `access_token`.
//!   They are alive and ask for a credential.
//! - With the real key of the user, `GetPlayerSummaries` answered **200** and
//!   `GetFamilyGroupForUser` answered **401**: the key is valid and the
//!   service of the family does not accept it. Valve does not document the
//!   service.
//! - `store.steampowered.com/pointssummary/ajaxgetasyncconfig` answers
//!   `{"success":1,"data":[]}` with no session. With a session, `data` carries
//!   `webapi_token`.
//!
//! The reference implementations are Lutris (`lutris/services/steamfamily.py`,
//! last change `192e214d` of 2026-06-21) and Hydra
//! (`src/main/services/steam-integration/`). Both send the token as
//! `access_token`. Hydra gives the difficulty that the documentation does not
//! give: a game with `exclude_reason` other than 0 is in the list and cannot
//! be lent, and `app_type` other than 1 is not a game. The fields of the answer
//! come from `SteamTracking/Protobufs`, `webui/service_familygroups.proto`.
//!
//! The 64-bit identifiers can come as text or as a number. The JSON of the Web
//! API gives them as text, and the parser accepts both, because a number
//! larger than 2^53 read as a float would be a different person.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use domain::{ConnectorError, FamilyApp, FamilyGroup, FamilyOwner, StoreSession};
use serde_json::Value;

use super::SteamConnector;

/// The page of the store that gives the token to the pages of the store.
pub const TOKEN_PAGE: &str = "https://store.steampowered.com/pointssummary/ajaxgetasyncconfig";

/// The sign in page of the store, which goes back to the page of the token.
pub const LOGIN_PAGE: &str =
    "https://store.steampowered.com/login/?redir=pointssummary%2Fajaxgetasyncconfig";

const STORE_HOST: &str = "store.steampowered.com";

/// `app_type` of a game. Hydra keeps only this type, and so does this parser.
const APP_TYPE_GAME: i64 = 1;

/// The most steamids that `GetPlayerSummaries` accepts in one request.
const SUMMARIES_BATCH: usize = 100;

/// The token of a session signed in to the store.
#[derive(Clone, PartialEq, Eq)]
pub struct StoreToken {
    pub token: String,
    /// The account that signed in, from the `sub` of the token. It is what lets
    /// the synchronisation refuse a session of another account.
    pub steam_id: Option<String>,
    /// Seconds from the epoch, from the `exp` of the token.
    pub expires_at: Option<i64>,
}

// The token opens the account: it must not appear in a log by accident.
impl std::fmt::Debug for StoreToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreToken")
            .field("token", &"…")
            .field("steam_id", &self.steam_id)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// What the page of the token says.
#[derive(Debug, PartialEq, Eq)]
pub enum TokenPage {
    Token(StoreToken),
    /// The store has no session: the user must sign in.
    SignedOut,
}

impl SteamConnector {
    /// The page where the script of the window may run. It is the page that
    /// gives the token, and never the page where the user types the password.
    pub fn is_token_page(url: &str) -> bool {
        reqwest::Url::parse(url).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str() == Some(STORE_HOST)
                && url.path().trim_end_matches('/') == "/pointssummary/ajaxgetasyncconfig"
        })
    }

    /// A page of the store that is not the sign in. After a sign in, the store
    /// can send the window to its home page and not to the page of the token:
    /// this is how the window knows that it must go to the token itself.
    pub fn is_store_page_after_login(url: &str) -> bool {
        reqwest::Url::parse(url).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str() == Some(STORE_HOST)
                && !url.path().starts_with("/login")
        })
    }

    /// Reads the text of the page of the token. `None` is a page that this
    /// program no longer understands, and not a page with no session.
    pub fn token_from_page(body: &str) -> Option<TokenPage> {
        let value: Value = serde_json::from_str(body).ok()?;
        let data = value.get("data")?;
        match data.get("webapi_token").and_then(Value::as_str) {
            Some(token) if !token.is_empty() => {
                let (steam_id, expires_at) = claims(token);
                Some(TokenPage::Token(StoreToken {
                    token: token.to_owned(),
                    steam_id,
                    expires_at,
                }))
            }
            // With no session the store gives `[]` and not an object.
            _ if data.as_array().is_some_and(Vec::is_empty) => Some(TokenPage::SignedOut),
            _ if data.as_object().is_some() => Some(TokenPage::SignedOut),
            _ => None,
        }
    }

    /// The family of the user, or the answer that there is none.
    pub async fn family_group(&self, token: &StoreToken) -> Result<FamilyGroup, ConnectorError> {
        let url = format!(
            "{}/IFamilyGroupsService/GetFamilyGroupForUser/v1/",
            self.api_base
        );
        let mut query = vec![("access_token", token.token.as_str())];
        if let Some(steam_id) = &token.steam_id {
            query.push(("steamid", steam_id.as_str()));
        }
        parse_family_group(&self.get(&url, &query).await?)
    }

    /// The games that the other members can lend.
    ///
    /// It asks with no games of the user, no excluded games and no other
    /// applications, and the parser filters again: the flags are not
    /// documented, and a game that cannot be lent must never become a mark.
    pub async fn shared_apps(
        &self,
        token: &StoreToken,
        group: &str,
        own_steam_id: &str,
    ) -> Result<Vec<FamilyApp>, ConnectorError> {
        let url = format!(
            "{}/IFamilyGroupsService/GetSharedLibraryApps/v1/",
            self.api_base
        );
        let body = self
            .get(
                &url,
                &[
                    ("access_token", token.token.as_str()),
                    ("family_groupid", group),
                    ("steamid", own_steam_id),
                    ("include_own", "0"),
                    ("include_excluded", "0"),
                    ("include_free", "0"),
                    ("include_non_games", "0"),
                    ("language", "english"),
                ],
            )
            .await?;
        parse_shared_apps(&body, own_steam_id)
    }

    /// The names of the profiles, with the API key of the user. This endpoint
    /// does accept the key, and the names of the profiles are public.
    pub async fn player_names(
        &self,
        session: &StoreSession,
        steam_ids: &[String],
    ) -> Result<HashMap<String, String>, ConnectorError> {
        let credential = Self::credential(session)?;
        let url = format!("{}/ISteamUser/GetPlayerSummaries/v2/", self.api_base);
        let mut names = HashMap::with_capacity(steam_ids.len());
        for batch in steam_ids.chunks(SUMMARIES_BATCH) {
            let ids = batch.join(",");
            let body = self
                .get(
                    &url,
                    &[
                        ("key", credential.api_key.as_str()),
                        ("steamids", ids.as_str()),
                    ],
                )
                .await?;
            names.extend(parse_player_names(&body)?);
        }
        Ok(names)
    }
}

/// The answer of `GetFamilyGroupForUser`.
pub fn parse_family_group(body: &str) -> Result<FamilyGroup, ConnectorError> {
    let response = response(body)?;
    if response
        .get("is_not_member_of_any_group")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(FamilyGroup::NotMember);
    }
    match response.get("family_groupid").and_then(id_text) {
        Some(id) if id != "0" => Ok(FamilyGroup::Member(id)),
        _ => Ok(FamilyGroup::NotMember),
    }
}

/// The answer of `GetSharedLibraryApps`, with only the games that another
/// member can lend.
pub fn parse_shared_apps(body: &str, own_steam_id: &str) -> Result<Vec<FamilyApp>, ConnectorError> {
    let response = response(body)?;
    let Some(apps) = response.get("apps") else {
        // A family with no game to lend gives no `apps` at all.
        return Ok(Vec::new());
    };
    let apps = apps
        .as_array()
        .ok_or_else(|| ConnectorError::Unexpected("response.apps is not a list".to_owned()))?;

    Ok(apps
        .iter()
        .filter_map(|app| {
            let app_id = app.get("appid").and_then(id_text)?;
            let lendable = app
                .get("exclude_reason")
                .and_then(Value::as_i64)
                .is_none_or(|reason| reason == 0);
            let game = app
                .get("app_type")
                .and_then(Value::as_i64)
                .is_none_or(|kind| kind == APP_TYPE_GAME);
            if !lendable || !game {
                return None;
            }
            let owners: Vec<FamilyOwner> = app
                .get("owner_steamids")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(id_text)
                .filter(|steam_id| steam_id != own_steam_id)
                .map(|steam_id| FamilyOwner {
                    steam_id,
                    name: None,
                })
                .collect();
            // Only the user has it: nobody can lend it to them.
            if owners.is_empty() {
                return None;
            }
            let title = app
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map_or_else(|| format!("Steam {app_id}"), str::to_owned);
            Some(FamilyApp {
                app_id,
                title,
                owners,
                igdb_id: None,
            })
        })
        .collect())
}

/// The names of `GetPlayerSummaries`, by steamid.
pub fn parse_player_names(body: &str) -> Result<HashMap<String, String>, ConnectorError> {
    let response = response(body)?;
    let players = response
        .get("players")
        .and_then(Value::as_array)
        .ok_or_else(|| ConnectorError::Unexpected("response.players is absent".to_owned()))?;
    Ok(players
        .iter()
        .filter_map(|player| {
            let steam_id = player.get("steamid").and_then(id_text)?;
            let name = player.get("personaname").and_then(Value::as_str)?;
            Some((steam_id, name.to_owned()))
        })
        .collect())
}

fn response(body: &str) -> Result<Value, ConnectorError> {
    let mut value: Value = serde_json::from_str(body)
        .map_err(|e| ConnectorError::Unexpected(format!("unreadable answer: {e}")))?;
    value
        .get_mut("response")
        .map(Value::take)
        .ok_or_else(|| ConnectorError::Unexpected("the answer has no response".to_owned()))
}

/// A 64-bit identifier as text, from text or from an integer. A float is not
/// an identifier: it has already lost the last digits.
fn id_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) => {
            Some(text.clone())
        }
        Value::Number(number) => number.as_u64().map(|n| n.to_string()),
        _ => None,
    }
}

/// The `sub` and the `exp` of the token. The token is a JWT, and its middle
/// part is JSON in base64. Nothing here checks the signature: Steam does that,
/// and the program only reads who signed in and until when.
fn claims(token: &str) -> (Option<String>, Option<i64>) {
    let Some(payload) = token.split('.').nth(1) else {
        return (None, None);
    };
    let Some(json) = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return (None, None);
    };
    (
        json.get("sub").and_then(id_text),
        json.get("exp").and_then(Value::as_i64),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A token with the shape of the tokens of Steam and an invented content.
    fn token(sub: &str, exp: i64) -> String {
        let payload =
            URL_SAFE_NO_PAD.encode(format!(r#"{{"iss":"r:X","sub":"{sub}","exp":{exp}}}"#));
        format!("eyJhbGciOiJFZERTQSJ9.{payload}.c2lnbmF0dXJl")
    }

    #[test]
    fn the_page_of_the_token_gives_the_account_and_the_expiry() {
        let body = format!(
            r#"{{"success":1,"data":{{"webapi_token":"{}"}}}}"#,
            token("76561198000000000", 1_760_600_000)
        );

        let Some(TokenPage::Token(found)) = SteamConnector::token_from_page(&body) else {
            panic!("a token");
        };
        assert_eq!(found.steam_id.as_deref(), Some("76561198000000000"));
        assert_eq!(found.expires_at, Some(1_760_600_000));
        assert!(!format!("{found:?}").contains(&found.token));
    }

    #[test]
    fn the_page_with_no_session_is_signed_out_and_not_unreadable() {
        // The real answer of 2026-10-15 with no session.
        assert_eq!(
            SteamConnector::token_from_page(r#"{"success":1,"data":[]}"#),
            Some(TokenPage::SignedOut)
        );
        assert_eq!(SteamConnector::token_from_page("<html>login</html>"), None);
        assert_eq!(SteamConnector::token_from_page(r#"{"success":1}"#), None);
    }

    #[test]
    fn a_token_that_is_not_a_jwt_is_still_a_token() {
        let Some(TokenPage::Token(found)) =
            SteamConnector::token_from_page(r#"{"success":1,"data":{"webapi_token":"opaque"}}"#)
        else {
            panic!("a token");
        };
        assert_eq!(found.steam_id, None);
        assert_eq!(found.expires_at, None);
    }

    #[test]
    fn the_script_runs_only_on_the_page_of_the_token() {
        assert!(SteamConnector::is_token_page(TOKEN_PAGE));
        assert!(SteamConnector::is_token_page(&format!(
            "{TOKEN_PAGE}?l=english"
        )));
        assert!(!SteamConnector::is_token_page(LOGIN_PAGE));
        assert!(!SteamConnector::is_token_page(
            "https://store.steampowered.com.evil.example/pointssummary/ajaxgetasyncconfig"
        ));
        assert!(!SteamConnector::is_token_page(
            "http://store.steampowered.com/pointssummary/ajaxgetasyncconfig"
        ));
    }

    #[test]
    fn the_sign_in_is_not_a_page_after_the_sign_in() {
        assert!(!SteamConnector::is_store_page_after_login(LOGIN_PAGE));
        assert!(SteamConnector::is_store_page_after_login(
            "https://store.steampowered.com/"
        ));
        assert!(!SteamConnector::is_store_page_after_login(
            "https://login.steampowered.com/jwt/refresh"
        ));
    }

    #[test]
    fn a_large_identifier_as_a_number_keeps_all_of_its_digits() {
        let body = r#"{"response":{"family_groupid":1234567890123456789}}"#;
        assert_eq!(
            parse_family_group(body).expect("group"),
            FamilyGroup::Member("1234567890123456789".to_owned())
        );
    }
}
