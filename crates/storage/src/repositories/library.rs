use domain::{GameId, PlayStatus};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::mapping::{game_id_from_text, status_from_str};
use crate::{Database, Result};

/// Una fila de la biblioteca: la ficha más todo lo que hay que enseñar de ella.
///
/// Se resuelve en una sola consulta a propósito. Pintar mil juegos haciendo una
/// consulta por juego para saber en qué tiendas está es la forma más fácil de
/// que la rejilla dé tirones.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LibraryRow {
    pub game_id: GameId,
    pub title: String,
    pub sort_title: String,
    pub cover_url: Option<String>,
    /// El resumen de IGDB. Falta en las fichas nacidas del título de la tienda.
    pub summary: Option<String>,
    pub release_year: Option<i32>,
    pub genres: Vec<String>,
    pub owned_stores: Vec<String>,
    pub wishlist_stores: Vec<String>,
    /// La imagen apaisada de la tienda, que es otra cosa que `cover_url`: IGDB
    /// sirve carátulas 3:4 y la tienda sirve cabeceras panorámicas.
    pub store_cover_url: Option<String>,
    pub store_url: Option<String>,
    pub playtime_minutes: i64,
    /// Última partida, en segundos desde la época. Solo lo publica Steam: GOG
    /// no da ni horas ni fecha, así que un juego solo de GOG lo tiene a `None`
    /// aunque se haya jugado.
    pub last_played_at: Option<i64>,
    pub status: Option<PlayStatus>,
    pub rating: Option<u8>,
    pub notes: Option<String>,
}

pub struct LibraryRepository<'a>(pub &'a Database);

impl LibraryRepository<'_> {
    pub async fn all(&self) -> Result<Vec<LibraryRow>> {
        sqlx::query(SQL)
            .fetch_all(self.0.pool())
            .await?
            .iter()
            .map(hydrate)
            .collect()
    }
}

/// La consulta de la biblioteca entera.
///
/// `CROSS JOIN` no es un producto cartesiano: en SQLite es la forma documentada
/// de fijar el orden de las tablas, y aquí hace falta. Con un `JOIN` normal el
/// planificador arrancaba desde `store_entry` por el índice `(kind, deleted_at)`
/// —que con `kind = 'owned'` no descarta casi nada— y comprobaba después contra
/// `game_link`: por cada juego recorría las copias de la biblioteca entera.
/// Forzando que mande `game_link`, cada subconsulta mira solo las copias de su
/// juego y busca la entrada por su clave. Con mil juegos son 10 ms en vez de
/// 839. El test `el_planificador_arranca_por_game_link` lo vigila.
const SQL: &str = "SELECT
                 g.id, g.canonical_title, g.sort_title, g.cover_url, g.released_at, g.genres,
                 g.summary,
                 us.status, us.rating, us.notes,
                 (SELECT GROUP_CONCAT(DISTINCT e.store) FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'owned' AND e.deleted_at IS NULL
                 ) AS owned_stores,
                 (SELECT GROUP_CONCAT(DISTINCT e.store) FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'wishlist' AND e.deleted_at IS NULL
                 ) AS wishlist_stores,
                 (SELECT COALESCE(SUM(e.playtime_minutes), 0) FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.deleted_at IS NULL
                 ) AS playtime_minutes,
                 -- La imagen y el enlace salen de la misma copia: el mismo
                 -- ORDER BY en las dos subconsultas es lo que evita enseñar la
                 -- cabecera de Steam con el enlace a GOG. Steam va primero
                 -- porque su `header.jpg` es una cabecera pensada para esto,
                 -- mientras que GOG sirve el logo del producto.
                 (SELECT e.cover_url FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'owned' AND e.deleted_at IS NULL
                     AND e.cover_url IS NOT NULL
                   ORDER BY e.store = 'steam' DESC, e.store LIMIT 1
                 ) AS store_cover_url,
                 (SELECT e.store_url FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'owned' AND e.deleted_at IS NULL
                     AND e.store_url IS NOT NULL
                   ORDER BY e.store = 'steam' DESC, e.store LIMIT 1
                 ) AS store_url,
                 -- Steam guarda la última partida dentro del JSON crudo desde
                 -- que existe el conector, así que se lee de ahí en vez de
                 -- materializar una columna: el dato se reescribe entero en
                 -- cada sincronización, y una columna propia habría que
                 -- migrarla y rellenarla para nada. El 0 de Steam significa
                 -- «nunca jugado», no «jugado en 1970».
                 (SELECT MAX(NULLIF(json_extract(e.raw, '$.rtime_last_played'), 0))
                    FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'owned' AND e.deleted_at IS NULL
                 ) AS last_played_at
             FROM game g
             LEFT JOIN user_state us ON us.game_id = g.id
             WHERE g.deleted_at IS NULL
             ORDER BY g.sort_title";

fn hydrate(row: &SqliteRow) -> Result<LibraryRow> {
    let status: Option<String> = row.get("status");
    let released: Option<time::OffsetDateTime> = row.get("released_at");

    Ok(LibraryRow {
        game_id: game_id_from_text(&row.get::<String, _>("id"))?,
        title: row.get("canonical_title"),
        sort_title: row.get("sort_title"),
        cover_url: row.get("cover_url"),
        summary: row.get("summary"),
        release_year: released.map(|date| date.year()),
        genres: serde_json::from_str(&row.get::<String, _>("genres")).unwrap_or_default(),
        owned_stores: split(row.get("owned_stores")),
        wishlist_stores: split(row.get("wishlist_stores")),
        store_cover_url: row.get("store_cover_url"),
        store_url: row.get("store_url"),
        playtime_minutes: row.get("playtime_minutes"),
        last_played_at: row.get("last_played_at"),
        status: status.as_deref().map(status_from_str).transpose()?,
        rating: row.get::<Option<i64>, _>("rating").map(|r| r as u8),
        notes: row.get("notes"),
    })
}

/// `GROUP_CONCAT` devuelve NULL cuando no hay filas, no una cadena vacía, y no
/// garantiza el orden: sin ordenar aquí, las insignias de tienda de un juego
/// podrían cambiar de sitio entre dos aperturas de la aplicación.
fn split(value: Option<String>) -> Vec<String> {
    let mut items: Vec<String> = value
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    items.sort();
    items
}

#[cfg(test)]
mod tests {
    use super::SQL;
    use crate::Database;
    use sqlx::Row;

    /// Los `CROSS JOIN` de la consulta parecen un descuido y no lo son: quitarlos
    /// no rompe ningún resultado, solo multiplica por ochenta lo que tarda, que
    /// es la clase de regresión que no se ve en verde ni en rojo.
    ///
    /// Se comprueba la forma del plan y no el tiempo, por lo mismo que
    /// `una_sola_consulta.rs` cuenta sentencias en vez de cronometrarlas: el
    /// reloj mide la carga de la máquina tanto como el código.
    #[tokio::test]
    async fn el_planificador_arranca_por_game_link() {
        let db = Database::in_memory().await.expect("base");

        let plan: Vec<String> = sqlx::query(&format!("EXPLAIN QUERY PLAN {SQL}"))
            .fetch_all(db.pool())
            .await
            .expect("plan")
            .iter()
            .map(|row| row.get::<String, _>("detail"))
            .collect();

        // Arrancar por este índice significa recorrer, para cada juego, todas
        // las copias de la biblioteca: `kind = 'owned'` no descarta nada.
        let culpables: Vec<&String> = plan
            .iter()
            .filter(|paso| paso.contains("store_entry_by_kind"))
            .collect();

        assert!(
            culpables.is_empty(),
            "alguna subconsulta vuelve a arrancar por store_entry en vez de por \
             game_link; revisa que no se haya perdido un CROSS JOIN:\n{culpables:#?}"
        );
    }
}
