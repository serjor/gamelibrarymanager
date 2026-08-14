//! Lectura de las respuestas de GOG. Sin IO: todo lo que hay aquí se prueba
//! con las respuestas grabadas en `tests/fixtures/`.

use std::collections::HashMap;

use domain::{ConnectorError, EntryKind, StoreAccountId, StoreEntry, StoreEntryId, StoreId};
use serde::Deserialize;
use time::OffsetDateTime;

/// Respuesta de `auth.gog.com/token`, tanto al canjear el código como al
/// refrescar.
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user_id: String,
}

/// Una entrada de `galaxy-library.gog.com/users/{id}/releases`.
///
/// `platform_id` importa más de lo que parece: Galaxy también lista lo que el
/// usuario tiene en otras tiendas conectadas, así que sin filtrar aquí el
/// conector de GOG acabaría inventándose copias de Steam.
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

/// Los identificadores propios de GOG de una página, y el testigo de la
/// siguiente si la hay.
pub fn parse_releases_page(body: &str) -> Result<(Vec<Release>, Option<String>), ConnectorError> {
    let page: ReleasesPage =
        serde_json::from_str(body).map_err(|e| ConnectorError::Unexpected(e.to_string()))?;

    let owned = page
        .items
        .into_iter()
        .filter(|item| item.owned && item.platform_id == PLATFORM_GOG)
        .collect();

    // Una página sin testigo es la última. GOG devuelve el campo a null en vez
    // de omitirlo, así que hay que tratar ambos casos igual.
    let next = page.next_page_token.filter(|token| !token.is_empty());
    Ok((owned, next))
}

/// El identificador de plataforma que usa Galaxy para la propia GOG.
const PLATFORM_GOG: &str = "gog";

#[derive(Debug, Deserialize)]
struct Product {
    id: serde_json::Value,
    title: Option<String>,
}

/// Títulos de `api.gog.com/products?ids=…`, indexados por identificador.
///
/// `id` llega como número, pero el resto del sistema trata `store_app_id` como
/// texto: se normaliza aquí y no en cada sitio que lo consulte.
pub fn parse_products(body: &str) -> HashMap<String, String> {
    let products: Vec<Product> = serde_json::from_str(body).unwrap_or_default();
    products
        .into_iter()
        .filter_map(|product| {
            let id = match product.id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s,
                _ => return None,
            };
            product.title.map(|title| (id, title))
        })
        .collect()
}

/// Nombre de la cuenta, de `users.gog.com/users/{id}`.
pub fn parse_username(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct UserData {
        username: Option<String>,
    }
    serde_json::from_str::<UserData>(body)
        .ok()
        .and_then(|user| user.username)
}

/// Convierte lo que dice GOG en entradas de biblioteca.
///
/// El título puede faltar: `api.gog.com/products` no conoce todos los
/// identificadores que devuelve Galaxy —los regalos y algunos paquetes no
/// están—, y quedarse sin la copia entera por no saber su nombre sería peor que
/// enseñarla con un nombre provisional que IGDB corregirá después.
pub fn to_entries(
    releases: &[Release],
    titles: &HashMap<String, String>,
    account_id: StoreAccountId,
) -> Vec<StoreEntry> {
    releases
        .iter()
        .map(|release| StoreEntry {
            id: StoreEntryId::new(),
            account_id,
            store: StoreId::Gog,
            store_app_id: release.external_id.clone(),
            kind: EntryKind::Owned,
            title: titles
                .get(&release.external_id)
                .cloned()
                .unwrap_or_else(|| format!("GOG {}", release.external_id)),
            // GOG no publica tiempo de juego en la biblioteca: lo lleva un
            // servicio aparte que solo responde por sesiones de Galaxy.
            playtime_minutes: None,
            acquired_at: release
                .owned_since
                .and_then(|seconds| OffsetDateTime::from_unix_timestamp(seconds).ok()),
            raw: serde_json::json!({
                "platform_id": release.platform_id,
                "external_id": release.external_id,
                "owned_since": release.owned_since,
            }),
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
        let titles = parse_products(r#"[{"id":1207658930,"title":"The Witcher 2"}]"#);
        assert_eq!(
            titles.get("1207658930").map(String::as_str),
            Some("The Witcher 2")
        );
    }
}
