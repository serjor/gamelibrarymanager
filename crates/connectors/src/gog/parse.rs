//! The reading of the GOG answers. With no IO: all of the code here is tested
//! with the answers recorded in `tests/fixtures/`.

use std::collections::HashMap;

use domain::{ConnectorError, EntryKind, StoreAccountId, StoreEntry, StoreEntryId, StoreId};
use serde::Deserialize;
use time::OffsetDateTime;

/// The answer of `auth.gog.com/token`, both when it exchanges the code and when
/// it refreshes.
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user_id: String,
}

/// An entry of `galaxy-library.gog.com/users/{id}/releases`.
///
/// `platform_id` is more important than it looks: Galaxy also lists what the
/// user has in other connected stores, thus without a filter here the GOG
/// connector would create Steam copies that do not exist.
#[derive(Debug, Deserialize)]
pub struct Release {
    pub platform_id: String,
    pub external_id: String,
    #[serde(default)]
    pub owned: bool,
    #[serde(default)]
    pub owned_since: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ReleasesPage {
    #[serde(default)]
    items: Vec<Release>,
    #[serde(default)]
    next_page_token: Option<String>,
}

/// The GOG identifiers of one page, and the token of the next page if there is
/// one.
pub fn parse_releases_page(body: &str) -> Result<(Vec<Release>, Option<String>), ConnectorError> {
    let page: ReleasesPage =
        serde_json::from_str(body).map_err(|e| ConnectorError::Unexpected(e.to_string()))?;

    let owned = page
        .items
        .into_iter()
        .filter(|item| item.owned && item.platform_id == PLATFORM_GOG)
        .collect();

    // A page with no token is the last page. GOG gives the field as null and
    // does not remove it, thus the two conditions must give the same result.
    let next = page.next_page_token.filter(|token| !token.is_empty());
    Ok((owned, next))
}

/// The platform identifier that Galaxy uses for GOG itself.
const PLATFORM_GOG: &str = "gog";

#[derive(Debug, Deserialize)]
struct Product {
    id: serde_json::Value,
    title: Option<String>,
    #[serde(default)]
    images: ProductImages,
    #[serde(default)]
    links: ProductLinks,
}

#[derive(Debug, Default, Deserialize)]
struct ProductImages {
    #[serde(default)]
    logo: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ProductLinks {
    #[serde(default)]
    product_card: Option<String>,
}

/// What GOG knows about a product and this code uses: its name, its cover and
/// its page. The matching does not use the last two; they exist so that the user
/// can compare when they must decide manually.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProductInfo {
    pub title: Option<String>,
    pub cover_url: Option<String>,
    pub store_url: Option<String>,
}

/// The data of `api.gog.com/products?ids=…`, indexed by identifier.
///
/// `id` comes as a number, but the remainder of the system holds `store_app_id`
/// as text: it is normalised here and not at each place that reads it.
pub fn parse_products(body: &str) -> HashMap<String, ProductInfo> {
    let products: Vec<Product> = serde_json::from_str(body).unwrap_or_default();
    products
        .into_iter()
        .filter_map(|product| {
            let id = match product.id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s,
                _ => return None,
            };
            Some((
                id,
                ProductInfo {
                    title: product.title,
                    cover_url: product.images.logo.map(|url| with_scheme(&url)),
                    store_url: product.links.product_card,
                },
            ))
        })
        .collect()
}

/// GOG gives the images with no scheme (`//images-4.gog-statics.com/…`). If you
/// keep them unchanged, the webview would resolve them against `tauri://` and
/// would load none of them.
fn with_scheme(url: &str) -> String {
    match url.strip_prefix("//") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_owned(),
    }
}

/// The name of the account, from `users.gog.com/users/{id}`.
pub fn parse_username(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct UserData {
        username: Option<String>,
    }
    serde_json::from_str::<UserData>(body)
        .ok()
        .and_then(|user| user.username)
}

/// Turns what GOG says into library entries.
///
/// The title can be absent: `api.gog.com/products` does not know all of the
/// identifiers that Galaxy gives back — the gifts and some packages are not
/// there — and to lose all of the copy because its name is unknown would be
/// worse than to show it with a temporary name that IGDB will correct later.
pub fn to_entries(
    releases: &[Release],
    products: &HashMap<String, ProductInfo>,
    account_id: StoreAccountId,
) -> Vec<StoreEntry> {
    releases
        .iter()
        .map(|release| {
            let product = products.get(&release.external_id);
            StoreEntry {
                id: StoreEntryId::new(),
                account_id,
                store: StoreId::Gog,
                store_app_id: release.external_id.clone(),
                kind: EntryKind::Owned,
                title: product
                    .and_then(|p| p.title.clone())
                    .unwrap_or_else(|| format!("GOG {}", release.external_id)),
                cover_url: product.and_then(|p| p.cover_url.clone()),
                store_url: product.and_then(|p| p.store_url.clone()),
                // GOG does not publish playtime in the library: a separate
                // service holds it and answers only for Galaxy sessions.
                playtime_minutes: None,
                acquired_at: release
                    .owned_since
                    .and_then(|seconds| OffsetDateTime::from_unix_timestamp(seconds).ok()),
                raw: serde_json::json!({
                    "platform_id": release.platform_id,
                    "external_id": release.external_id,
                    "owned_since": release.owned_since,
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descarta_lo_que_no_es_de_gog() {
        // Galaxy lista también las tiendas conectadas: si esto no se filtra, el
        // conector de GOG duplicaría la biblioteca de Steam.
        let body = r#"{"items":[
            {"platform_id":"gog","external_id":"1207658930","owned":true,"owned_since":1500000000},
            {"platform_id":"steam","external_id":"632470","owned":true,"owned_since":null}
        ],"next_page_token":null}"#;

        let (releases, next) = parse_releases_page(body).expect("página válida");
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].external_id, "1207658930");
        assert_eq!(next, None);
    }

    #[test]
    fn descarta_lo_que_no_se_posee() {
        let body = r#"{"items":[
            {"platform_id":"gog","external_id":"1","owned":false}
        ]}"#;
        let (releases, _) = parse_releases_page(body).expect("página válida");
        assert!(releases.is_empty());
    }

    #[test]
    fn el_identificador_numerico_se_lee_como_texto() {
        let productos = parse_products(r#"[{"id":1207658930,"title":"The Witcher 2"}]"#);
        assert_eq!(
            productos.get("1207658930").and_then(|p| p.title.as_deref()),
            Some("The Witcher 2")
        );
    }

    #[test]
    fn la_imagen_sin_esquema_se_completa() {
        // GOG las sirve como `//images-4.gog-statics.com/…`. Tal cual, el
        // webview las resolvería contra `tauri://` y no cargaría ninguna.
        let productos = parse_products(
            r#"[{"id":1,"title":"X",
                 "images":{"logo":"//images-4.gog-statics.com/abc_glx_logo.jpg"},
                 "links":{"product_card":"https://www.gog.com/game/x"}}]"#,
        );
        let producto = productos.get("1").expect("el producto");
        assert_eq!(
            producto.cover_url.as_deref(),
            Some("https://images-4.gog-statics.com/abc_glx_logo.jpg")
        );
        assert_eq!(
            producto.store_url.as_deref(),
            Some("https://www.gog.com/game/x")
        );
    }

    #[test]
    fn un_producto_sin_imagenes_ni_enlaces_no_rompe() {
        let productos = parse_products(r#"[{"id":1,"title":"X"}]"#);
        let producto = productos.get("1").expect("el producto");
        assert_eq!(producto.cover_url, None);
        assert_eq!(producto.store_url, None);
    }
}
