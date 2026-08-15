//! Reading of the Epic answers. No IO here: everything in this file runs
//! against the responses recorded in `tests/fixtures/`.

use std::collections::HashMap;

use domain::{ConnectorError, EntryKind, StoreAccountId, StoreEntry, StoreEntryId, StoreId};
use serde::Deserialize;

/// Answer of `account/api/oauth/token`, both for the code exchange and for the
/// refresh.
///
/// Epic mixes two spellings in the same object: the OAuth fields are
/// `snake_case` and its own fields are `camelCase`. A blanket `rename_all`
/// silently drops half of them, so each name is written out.
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

/// The body Epic sends with a 4xx on the token endpoint.
#[derive(Debug, Default, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    /// Only present when the account itself must do something —accept new
    /// terms, verify the mail— before any application can authorise.
    #[serde(rename = "continuationUrl")]
    pub continuation_url: Option<String>,
}

/// The authorisation code, read from the body of `id/api/redirect`.
///
/// Epic does not put the code in a redirection: the page answers JSON, and this
/// field is null until the user has a session.
#[derive(Debug, Deserialize)]
pub struct AuthorizationResponse {
    #[serde(rename = "authorizationCode")]
    pub authorization_code: Option<String>,
}

/// One entry of `launcher/api/public/assets/{platform}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub app_name: String,
    pub namespace: String,
    pub catalog_item_id: String,
    #[serde(default)]
    pub build_version: String,
}

/// The namespace of the Unreal Marketplace.
///
/// The same endpoint hands out the engine assets of the account, and there are
/// thousands of them. They are not games and they would bury the library, so
/// legendary skips this namespace by default and so does this connector.
const NAMESPACE_UNREAL: &str = "ue";

/// What the account owns on the launcher, minus the Unreal assets.
pub fn parse_assets(body: &str) -> Result<Vec<Asset>, ConnectorError> {
    let assets: Vec<Asset> =
        serde_json::from_str(body).map_err(|e| ConnectorError::Unexpected(e.to_string()))?;

    Ok(assets
        .into_iter()
        .filter(|asset| asset.namespace != NAMESPACE_UNREAL)
        .collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogItem {
    title: Option<String>,
    #[serde(default)]
    key_images: Vec<KeyImage>,
    #[serde(default)]
    categories: Vec<Category>,
    /// Only DLC carry this field. It is how legendary tells an add-on from a
    /// game, and there is no other flag that says it.
    #[serde(default)]
    main_game_item: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct KeyImage {
    #[serde(rename = "type")]
    kind: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Category {
    path: Option<String>,
}

/// What the catalogue knows about an item and this connector uses: its name and
/// its cover. The matching only reads the name; the cover is there so that a
/// person can compare when the queue asks.
///
/// There is no page of the store here, and it is not an oversight. See the
/// module documentation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemInfo {
    pub title: Option<String>,
    pub cover_url: Option<String>,
    /// False for DLC and for mods. They are owned, they have their own asset,
    /// and they are not games: listing them would put an entry in the library
    /// for every expansion of every game.
    pub is_game: bool,
}

/// Cover candidates, in the order in which they are wanted.
///
/// The launcher catalogue answers with the `Diesel*` family and the store one
/// with the `Offer*` family. Both reach this code —the same item is read
/// through two doors— so both are accepted and the vertical ones win: they are
/// the box art, and the rest are banners that look wrong in a grid.
const COVER_TYPES: [&str; 4] = [
    "DieselGameBoxTall",
    "OfferImageTall",
    "DieselGameBox",
    "Thumbnail",
];

/// Path of the category that marks a modification of another game.
const CATEGORY_MODS: &str = "mods";

/// One entry of `catalog/api/shared/namespace/{ns}/offers`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Offer {
    id: String,
    #[serde(default)]
    offer_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OfferPage {
    #[serde(default)]
    elements: Vec<Offer>,
}

/// The offer type that sells the game itself, as opposed to its add-ons,
/// its currency packs and its season passes.
const OFFER_BASE_GAME: &str = "BASE_GAME";

/// The identifier of the offer that sells the game of a namespace, and only
/// when there is exactly one.
///
/// IGDB indexes Epic by offer, so this is the identifier that crosses. It
/// answers `None` when the namespace has no base game or has two, and the
/// second case is the one that matters: `Chivalry 2` and
/// `Chivalry 2 Special Edition` live in one namespace, nothing in the answer
/// says which one the account owns, and a wrong choice attaches the copy to the
/// wrong card of IGDB. A copy that goes to the review queue costs the user one
/// click. A copy attached to the wrong card costs the state that they wrote.
///
/// Measured on 2026-08-15 over 90 namespaces of a real library: 85 have exactly
/// one base game, 1 has two, and 4 have none.
pub fn parse_base_game_offer(body: &str) -> Option<String> {
    let page: OfferPage = serde_json::from_str(body).unwrap_or_default();

    let mut base = page
        .elements
        .into_iter()
        .filter(|offer| offer.offer_type.as_deref() == Some(OFFER_BASE_GAME));

    match (base.next(), base.next()) {
        (Some(unica), None) => Some(unica.id),
        _ => None,
    }
}

/// Data of `catalog/api/shared/namespace/{ns}/bulk/items`, indexed by catalogue
/// identifier, which is the key of the answer itself.
pub fn parse_items(body: &str) -> HashMap<String, ItemInfo> {
    let items: HashMap<String, CatalogItem> = serde_json::from_str(body).unwrap_or_default();

    items
        .into_iter()
        .map(|(id, item)| {
            let info = ItemInfo {
                title: item.title.clone(),
                cover_url: cover_url(&item),
                is_game: is_game(&item),
            };
            (id, info)
        })
        .collect()
}

fn is_game(item: &CatalogItem) -> bool {
    let is_mod = item
        .categories
        .iter()
        .any(|category| category.path.as_deref() == Some(CATEGORY_MODS));

    item.main_game_item.is_none() && !is_mod
}

fn cover_url(item: &CatalogItem) -> Option<String> {
    COVER_TYPES.iter().find_map(|wanted| {
        item.key_images
            .iter()
            .find(|image| image.kind.as_deref() == Some(*wanted))
            .and_then(|image| image.url.clone())
    })
}

/// Turns what Epic says into library entries.
///
/// An asset without a catalogue item survives with a provisional name. Losing
/// the whole copy because one request out of two hundred failed would be worse
/// than showing a name that IGDB corrects later, and it is the same trade the
/// GOG connector makes. The price is that a DLC whose item did not arrive comes
/// in as if it were a game: it ends up in the review queue, which is where a
/// person can see it.
pub fn to_entries(
    assets: &[Asset],
    items: &HashMap<String, ItemInfo>,
    offers: &HashMap<String, String>,
    account_id: StoreAccountId,
) -> Vec<StoreEntry> {
    assets
        .iter()
        .filter(|asset| {
            items
                .get(&asset.catalog_item_id)
                .is_none_or(|item| item.is_game)
        })
        .map(|asset| {
            let item = items.get(&asset.catalog_item_id);
            StoreEntry {
                id: StoreEntryId::new(),
                account_id,
                store: StoreId::Epic,
                // `appName` and not `catalogItemId`: it is the identifier
                // legendary and the launcher itself use, it is stable, and it
                // is the one that appears in any report the user may read.
                store_app_id: asset.app_name.clone(),
                kind: EntryKind::Owned,
                title: item
                    .and_then(|item| item.title.clone())
                    .unwrap_or_else(|| format!("Epic {}", asset.app_name)),
                cover_url: item.and_then(|item| item.cover_url.clone()),
                // Epic is the only store of the three whose copy has no page to
                // point at. The module documentation says why, with the count.
                store_url: None,
                // Epic publishes neither played time nor purchase date on the
                // launcher assets. The date does travel in the library service,
                // which needs one more request per page and gives nothing else
                // this connector uses.
                playtime_minutes: None,
                acquired_at: None,
                // This layer does not use `offerId`: it is what IGDB indexes,
                // and it travels here so that the matching crosses without
                // asking Epic again. It is absent when the namespace has no
                // single base game offer, and the copy then goes by title.
                raw: serde_json::json!({
                    "appName": asset.app_name,
                    "namespace": asset.namespace,
                    "catalogItemId": asset.catalog_item_id,
                    "offerId": offers.get(&asset.namespace),
                    "buildVersion": asset.build_version,
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_the_unreal_marketplace() {
        // The same endpoint hands out the engine assets of the account, and an
        // account that uses Unreal has thousands of them.
        let body = r#"[
            {"appName":"Sugar","namespace":"d5241c76f178492ea1540fce45616757",
             "catalogItemId":"1e8bda5cdbea4b7d81a8c733e2a48f18","buildVersion":"1.0"},
            {"appName":"BlueprintMaterial","namespace":"ue",
             "catalogItemId":"aaaa","buildVersion":"1.0"}
        ]"#;

        let assets = parse_assets(body).expect("valid list");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].app_name, "Sugar");
    }

    #[test]
    fn a_dlc_is_not_a_game() {
        // `mainGameItem` is the only field that says it, and without this
        // filter every expansion would get its own card in the library.
        let items =
            parse_items(r#"{"abc":{"title":"Some Expansion","mainGameItem":{"id":"xyz"}}}"#);
        assert!(!items["abc"].is_game);
    }

    #[test]
    fn a_mod_is_not_a_game_either() {
        let items = parse_items(
            r#"{"abc":{"title":"A Mod","categories":[{"path":"mods"},{"path":"applications"}]}}"#,
        );
        assert!(!items["abc"].is_game);
    }

    #[test]
    fn the_vertical_cover_wins_over_the_banner() {
        let items = parse_items(
            r#"{"abc":{"title":"X","keyImages":[
                {"type":"OfferImageWide","url":"https://cdn1.epicgames.com/wide"},
                {"type":"OfferImageTall","url":"https://cdn1.epicgames.com/tall"}
            ]}}"#,
        );
        assert_eq!(
            items["abc"].cover_url.as_deref(),
            Some("https://cdn1.epicgames.com/tall")
        );
    }

    #[test]
    fn an_item_without_images_does_not_break() {
        let items = parse_items(r#"{"abc":{"title":"X"}}"#);
        assert_eq!(items["abc"].cover_url, None);
        assert!(items["abc"].is_game);
    }

    #[test]
    fn an_asset_without_its_item_keeps_a_provisional_name() {
        let assets = parse_assets(
            r#"[{"appName":"Sugar","namespace":"ns","catalogItemId":"abc","buildVersion":"1.0"}]"#,
        )
        .expect("valid list");

        let entries = to_entries(
            &assets,
            &HashMap::new(),
            &HashMap::new(),
            StoreAccountId::new(),
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Epic Sugar");
        assert_eq!(entries[0].store_app_id, "Sugar");
        // Without an offer the field is null, not absent: the matching then
        // reads it as "no identifier" instead of as a shape it does not know.
        assert!(entries[0].raw["offerId"].is_null());
    }

    #[test]
    fn one_base_game_in_the_namespace_is_an_identifier() {
        let body = r#"{"elements":[
            {"id":"OFERTA","offerType":"BASE_GAME","title":"Kena"},
            {"id":"DLC","offerType":"DLC","title":"Kena - Digital Deluxe"},
            {"id":"MONEDA","offerType":"VIRTUAL_CURRENCY","title":"1000 gemas"}
        ]}"#;

        assert_eq!(parse_base_game_offer(body), Some("OFERTA".to_owned()));
    }

    #[test]
    fn two_base_games_in_the_namespace_are_not_an_identifier() {
        // The real case that this rule exists for: `Chivalry 2` and its special
        // edition share a namespace, and nothing here says which one is owned.
        let body = r#"{"elements":[
            {"id":"NORMAL","offerType":"BASE_GAME","title":"Chivalry 2"},
            {"id":"ESPECIAL","offerType":"BASE_GAME","title":"Chivalry 2 Special Edition"}
        ]}"#;

        assert_eq!(parse_base_game_offer(body), None);
    }

    #[test]
    fn a_namespace_without_a_base_game_is_not_an_identifier() {
        assert_eq!(parse_base_game_offer(r#"{"elements":[]}"#), None);
        assert_eq!(parse_base_game_offer("not json"), None);
    }
}
