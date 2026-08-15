use domain::{GameId, PlayStatus};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::mapping::{game_id_from_text, status_from_str};
use crate::{Database, Result};

/// A row of the library: the game record and all of the data to show with it.
///
/// It is resolved in one query, and that is deliberate. To show one thousand
/// games with one query for each game to find its stores is the easiest way to
/// make the grid jump.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LibraryRow {
    pub game_id: GameId,
    pub title: String,
    pub sort_title: String,
    pub cover_url: Option<String>,
    /// The summary from IGDB. It is absent in the records that come from the
    /// title of the store.
    pub summary: Option<String>,
    pub release_year: Option<i32>,
    pub genres: Vec<String>,
    pub owned_stores: Vec<String>,
    pub wishlist_stores: Vec<String>,
    /// The horizontal image of the store, which is different from `cover_url`:
    /// IGDB gives 3:4 covers and the store gives wide headers.
    pub store_cover_url: Option<String>,
    pub store_url: Option<String>,
    pub playtime_minutes: i64,
    /// The last time played, in seconds from the epoch. Only Steam publishes
    /// it: GOG gives neither hours nor date, thus a game that is only from GOG
    /// keeps `None` even if the user has played it.
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

/// The query for all of the library.
///
/// `CROSS JOIN` is not a Cartesian product: in SQLite it is the documented way
/// to set the order of the tables, and it is necessary here. With a usual
/// `JOIN`, the planner started at `store_entry` through the `(kind, deleted_at)`
/// index — which with `kind = 'owned'` removes almost nothing — and then
/// compared against `game_link`: for each game it went through the copies of all
/// of the library. When you make `game_link` control the plan, each subquery
/// looks only at the copies of its own game and finds the entry by its key. With
/// one thousand games that is 10 ms and not 839. The test
/// `el_planificador_arranca_por_game_link` makes sure of it.
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
                 -- The image and the link come from the same copy: the same
                 -- ORDER BY in the two subqueries is what prevents a Steam
                 -- header with a GOG link. Steam is first because its
                 -- `header.jpg` is a header made for this, while GOG gives the
                 -- logo of the product.
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
                 -- Steam has kept the last time played in the raw JSON since
                 -- the connector started, thus it is read from there and not
                 -- materialised in a column: the data is written again complete
                 -- at each synchronisation, and a column of its own would need
                 -- a migration and a fill for no gain. The 0 from Steam means
                 -- never played, not played in 1970.
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

/// `GROUP_CONCAT` gives back NULL when there are no rows, not an empty string,
/// and it does not make sure of the order: without a sort here, the store badges
/// of a game could change position between two starts of the application.
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
