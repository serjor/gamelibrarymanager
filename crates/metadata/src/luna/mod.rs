//! The Amazon Luna client: the catalogue that Prime includes.
//!
//! One question, and no credential. The web page of Luna asks for its
//! catalogue with no session, and this client asks in the same way. The user
//! does not connect an Amazon account and the application never asks for its
//! password.
//!
//! ## The endpoint (examined on 2026-10-04)
//!
//! Luna has no public API, and neither Playnite nor Heroic reads it. The
//! reference is the web page `luna.amazon.es` itself: its `ProxyClient`
//! (`GET_PAGE_PATH = "getPage"`, `requestUnauthenticated`) and the requests it
//! makes when you select the "Luna Standard" filter in `/browse`.
//!
//! - `POST https://proxy-prod.eu-west-1.tempo.digital.a2z.com/getPage` is
//!   **alive** with an empty `x-amz-access-token`. The body is a
//!   `searchContext` with `query: "included_with_prime"` and the `pageContext`
//!   `multistate_browse_results`. It answers `200` with
//!   `pageType: browse_results_included_with_prime`: 89 games in one answer,
//!   with no field for pages.
//! - `us-east-1` answers the same. This client uses the host near the country.
//! - `x-amz-locale` is required: with no locale it answers `400`. With no
//!   `x-amz-marketplace-id` it answers `500`.
//! - **Amazon selects the territory from the connection, not from the
//!   headers.** The eleven marketplaces of the list below answer `200` with the
//!   same 89 games, and each answer says `territory=ES` in the address of the
//!   game. An invented marketplace gives the same catalogue. Thus the country
//!   of the user decides the headers and the address of the links, and the
//!   territory that the answer gives is what the interface shows.
//! - With `es_ES`, 16 of the 89 titles are in Spanish ("Aventureros al Tren®"
//!   for "Ticket to Ride®"). The identifiers are the same with `en_GB`. The
//!   English titles are the titles of IGDB and of the stores, thus this client
//!   always asks in English.
//! - Each game is a widget of type `GAME_TILE` inside a `GRID`. Its
//!   `presentationData` is JSON inside a string, with `gameId` and `title`.
//!   The action of the tile has `target: "/game/<slug>/<ASIN>"`. One tile of
//!   the 89, Fortnite, has `target: "/game/fortnite"` with no ASIN. Thus the
//!   identity of an offer is `gameId`, which each tile has, and the ASIN is
//!   optional.

mod parse;

use domain::LunaCatalog;

use crate::{MetadataError, Result};

/// The body of the request, as the web page sends it.
///
/// `clientContext` and `inputContext` describe a browser. The server does not
/// need more than this to answer, and the web page sends no less.
const PRIME_CATALOG_REQUEST: &str = r#"{
    "timeout": 10000,
    "featureScheme": "WEB_V1",
    "searchContext": {"query": "included_with_prime", "sort": "TITLE_A_TO_Z", "filtersList": []},
    "pageContext": {"pageType": "multistate_browse_results", "pageId": "default"},
    "clientContext": {"browserMetadata": {
        "browserClientRole": "browser", "browserType": "Chrome", "browserVersion": "152.0",
        "deviceModel": "unknown", "deviceType": "unknown", "osName": "Linux", "osVersion": "0"
    }},
    "inputContext": {"gamepadTypes": []},
    "dynamicFeatures": []
}"#;

const HOST_EU: &str = "https://proxy-prod.eu-west-1.tempo.digital.a2z.com";
const HOST_US: &str = "https://proxy-prod.us-east-1.tempo.digital.a2z.com";

/// A country where Luna operates, with what a request for it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LunaRegion {
    /// The ISO code of two letters.
    pub country: &'static str,
    /// The domain of the Luna web page for that country, which is where the
    /// links of the interface go.
    pub domain: &'static str,
    marketplace: &'static str,
    locale: &'static str,
    host: &'static str,
}

/// The countries that answered `200` on 2026-10-04. These are the countries
/// that the web page of Luna links to. A country that is not here is not
/// offered: no person examined it.
const REGIONS: &[LunaRegion] = &[
    region(
        "BE",
        "luna.amazon.com.be",
        "AMEN7PMS3EDWL",
        "en_GB",
        HOST_EU,
    ),
    region("CA", "luna.amazon.ca", "A2EUQ1WTGCTBG2", "en_CA", HOST_US),
    region("DE", "luna.amazon.de", "A1PA6795UKMFR9", "en_GB", HOST_EU),
    region("ES", "luna.amazon.es", "A1RKKUPIHCS9HS", "en_GB", HOST_EU),
    region("FR", "luna.amazon.fr", "A13V1IB3VIYZZH", "en_GB", HOST_EU),
    region(
        "GB",
        "luna.amazon.co.uk",
        "A1F83G8C2ARO7P",
        "en_GB",
        HOST_EU,
    ),
    region("IT", "luna.amazon.it", "APJ6JRA9NG5V4", "en_GB", HOST_EU),
    region("NL", "luna.amazon.nl", "A1805IZSGTT6HS", "en_GB", HOST_EU),
    region("PL", "luna.amazon.pl", "A1C3SOZRARQ6R3", "en_GB", HOST_EU),
    region("SE", "luna.amazon.se", "A2NODRKZP88ZB9", "en_GB", HOST_EU),
    region("US", "luna.amazon.com", "ATVPDKIKX0DER", "en_US", HOST_US),
];

const fn region(
    country: &'static str,
    domain: &'static str,
    marketplace: &'static str,
    locale: &'static str,
    host: &'static str,
) -> LunaRegion {
    LunaRegion {
        country,
        domain,
        marketplace,
        locale,
        host,
    }
}

impl LunaRegion {
    pub fn supported() -> &'static [LunaRegion] {
        REGIONS
    }

    /// The region of a country code, in any case.
    pub fn find(country: &str) -> Option<LunaRegion> {
        let country = country.trim();
        REGIONS
            .iter()
            .find(|region| region.country.eq_ignore_ascii_case(country))
            .copied()
    }

    /// The address of the page of a game, from the path of its offer.
    pub fn game_url(&self, path: &str) -> String {
        format!("https://{}{path}", self.domain)
    }
}

pub struct LunaClient {
    http: reqwest::Client,
    base: Option<String>,
}

impl LunaClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, base: None }
    }

    /// Sends the calls to a different host. It exists for the tests.
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = Some(base.into());
        self
    }

    /// The games that Prime includes today.
    pub async fn prime_catalog(&self, region: &LunaRegion) -> Result<LunaCatalog> {
        let base = self.base.as_deref().unwrap_or(region.host);
        let response = self
            .http
            .post(format!("{base}/getPage"))
            .header("content-type", "application/json")
            .header("x-amz-access-token", "")
            .header("x-amz-platform", "web")
            .header("x-amz-locale", region.locale)
            .header("x-amz-marketplace-id", region.marketplace)
            .header("x-amz-country-of-residence", region.country)
            .body(PRIME_CATALOG_REQUEST)
            .send()
            .await
            .map_err(|e| MetadataError::Transport(e.to_string()))?;

        let body = match response.status().as_u16() {
            200 => response
                .text()
                .await
                .map_err(|e| MetadataError::Transport(e.to_string()))?,
            429 => return Err(MetadataError::RateLimited),
            other => return Err(MetadataError::Unexpected(format!("HTTP {other}"))),
        };

        parse::parse_catalog(&body)
    }
}
