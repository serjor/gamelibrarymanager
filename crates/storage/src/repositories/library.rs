use domain::{GameId, LunaIndex, LunaMark, LunaOffer, ManualWish, PlayStatus, TimeToBeat};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::mapping::{game_id_from_text, game_id_to_text, status_from_str};
use crate::{Database, Result, StorageError};

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
    /// How long the game takes, as the players report it to IGDB. Absent in
    /// the records with no IGDB identity, in the records that no pass has asked
    /// for yet, and when nobody reported a time.
    pub time_to_beat: Option<TimeToBeat>,
    pub owned_stores: Vec<String>,
    pub wishlist_stores: Vec<String>,
    /// Wishes written by the user, not copies reported by a store.
    pub manual_wishes: Vec<ManualWish>,
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
    /// The game is in the catalogue of Luna that Prime includes. Absent when
    /// Luna is switched off.
    ///
    /// It is not written when it is absent, and the export removes it: it is
    /// data of Amazon that expires, not data of the library, and a file that
    /// the user takes out must not change because Luna is on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub luna: Option<LunaMark>,
    /// The titles of the live copies of the record, as each store writes them.
    /// They are only for the match with Luna and the interface never sees them.
    #[serde(skip)]
    pub store_titles: Vec<String>,
}

pub struct LibraryRepository<'a>(pub &'a Database);

impl LibraryRepository<'_> {
    pub async fn all(&self) -> Result<Vec<LibraryRow>> {
        rows(self.0, None).await
    }

    /// The row of one game, for the answer of a save.
    ///
    /// It gives `None` for a game that the library no longer shows: a record
    /// with a logical delete is not in `all()` either, and the answer of a save
    /// must not say something that the list denies.
    pub async fn one(&self, game_id: GameId) -> Result<Option<LibraryRow>> {
        Ok(rows(self.0, Some(game_id)).await?.pop())
    }
}

/// The two ways to ask go through here, thus they go through the same `SQL`.
///
/// A row that a save gives back cannot become different from the row of the
/// list: there is no second query where one of them could start to build the
/// badges, the hours or the store link in a different way.
async fn rows(db: &Database, game_id: Option<GameId>) -> Result<Vec<LibraryRow>> {
    let mut rows = sqlx::query(SQL)
        .bind(game_id.map(game_id_to_text))
        .fetch_all(db.pool())
        .await?
        .iter()
        .map(hydrate)
        .collect::<Result<Vec<_>>>()?;
    mark_luna(db, &mut rows).await?;
    Ok(rows)
}

/// Marks the rows that are in the Luna catalogue.
///
/// One statement more, whatever the size of the library: the catalogue comes
/// once and the match happens here and not in SQL, because the normalisation
/// of a title is a rule of the domain that SQL cannot repeat. The match is done
/// at each read and not kept: a wish added after the last refresh is matched at
/// once, and a change to the rule needs no migration.
async fn mark_luna(db: &Database, rows: &mut [LibraryRow]) -> Result<()> {
    // With no region the `LEFT JOIN` gives no row, and with a region and no
    // catalogue it gives one row with no title.
    let catalog = sqlx::query(
        "SELECT r.site, c.product_id, c.title, c.path, c.asin
           FROM luna_region r
           LEFT JOIN luna_catalog c
          ORDER BY c.title, c.product_id",
    )
    .fetch_all(db.pool())
    .await?;

    let Some(site) = catalog.first().map(|row| row.get::<String, _>("site")) else {
        return Ok(());
    };
    let offers: Vec<LunaOffer> = catalog
        .iter()
        .filter_map(|row| {
            Some(LunaOffer {
                product_id: row.get::<Option<String>, _>("product_id")?,
                title: row.get("title"),
                path: row.get("path"),
                asin: row.get("asin"),
            })
        })
        .collect();

    let index = LunaIndex::new(&offers);
    for row in rows {
        let titles =
            std::iter::once(row.title.as_str()).chain(row.store_titles.iter().map(String::as_str));
        row.luna = index.find(titles).map(|offer| LunaMark {
            title: offer.title.clone(),
            url: format!("https://{site}{}", offer.path),
        });
    }
    Ok(())
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
/// `the_planner_starts_at_game_link` makes sure of it.
const SQL: &str = "SELECT
                 g.id, g.canonical_title, g.sort_title, g.cover_url, g.released_at, g.genres,
                 g.summary,
                 t.hastily, t.normally, t.completely, t.submissions,
                 us.status, us.rating, us.notes,
                 (SELECT GROUP_CONCAT(DISTINCT e.store) FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'owned' AND e.deleted_at IS NULL
                 ) AS owned_stores,
                 (SELECT GROUP_CONCAT(DISTINCT e.store) FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.kind = 'wishlist' AND e.deleted_at IS NULL
                 ) AS wishlist_stores,
                 (SELECT json_group_array(json_object(
                       'id', m.id, 'game_id', m.game_id,
                       'family', m.family, 'model', m.model))
                    FROM (SELECT id, game_id, family, model FROM manual_wish
                           WHERE game_id = g.id AND deleted_at IS NULL
                           ORDER BY family, model_key) m
                 ) AS manual_wishes,
                 (SELECT json_group_array(DISTINCT e.title) FROM game_link l
                    CROSS JOIN store_entry e ON e.id = l.store_entry_id
                   WHERE l.game_id = g.id AND e.deleted_at IS NULL
                 ) AS store_titles,
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
             -- The key of the cache: one row at the most for each game.
             LEFT JOIN igdb_time_to_beat t ON t.igdb_id = g.igdb_id
             -- The one parameter selects one game, and NULL asks for all of
             -- them. The `WHERE` is examined before the subqueries of the
             -- columns, thus one game costs one pass over `game` and the
             -- subqueries of one row.
             WHERE g.deleted_at IS NULL AND (?1 IS NULL OR g.id = ?1)
             ORDER BY g.sort_title";

fn hydrate(row: &SqliteRow) -> Result<LibraryRow> {
    let status: Option<String> = row.get("status");
    let released: Option<time::OffsetDateTime> = row.get("released_at");
    let manual_json: String = row.get("manual_wishes");
    let manual_wishes = serde_json::from_str(&manual_json).map_err(|_| StorageError::Corrupt {
        column: "manual_wishes",
        value: manual_json,
    })?;
    let titles_json: String = row.get("store_titles");
    let store_titles = serde_json::from_str(&titles_json).map_err(|_| StorageError::Corrupt {
        column: "store_titles",
        value: titles_json,
    })?;

    let time_to_beat = TimeToBeat {
        hastily: row.get("hastily"),
        normally: row.get("normally"),
        completely: row.get("completely"),
        submissions: row.get::<Option<i64>, _>("submissions").unwrap_or(0),
    };

    Ok(LibraryRow {
        game_id: game_id_from_text(&row.get::<String, _>("id"))?,
        title: row.get("canonical_title"),
        sort_title: row.get("sort_title"),
        cover_url: row.get("cover_url"),
        summary: row.get("summary"),
        release_year: released.map(|date| date.year()),
        genres: serde_json::from_str(&row.get::<String, _>("genres")).unwrap_or_default(),
        time_to_beat: (!time_to_beat.is_empty()).then_some(time_to_beat),
        owned_stores: split(row.get("owned_stores")),
        wishlist_stores: split(row.get("wishlist_stores")),
        manual_wishes,
        store_cover_url: row.get("store_cover_url"),
        store_url: row.get("store_url"),
        playtime_minutes: row.get("playtime_minutes"),
        last_played_at: row.get("last_played_at"),
        status: status.as_deref().map(status_from_str).transpose()?,
        rating: row.get::<Option<i64>, _>("rating").map(|r| r as u8),
        notes: row.get("notes"),
        luna: None,
        store_titles,
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

    /// The `CROSS JOIN` clauses of the query look like a mistake and they are
    /// not: to remove them breaks no result, it only multiplies the time by
    /// eighty, which is the kind of regression that you see neither in green nor
    /// in red.
    ///
    /// The test examines the shape of the plan and not the time, for the same
    /// reason that `one_query.rs` counts statements and does not measure their
    /// time: a clock measures the load of the machine as much as the code.
    #[tokio::test]
    async fn the_planner_starts_at_game_link() {
        let db = Database::in_memory().await.expect("database");

        let plan: Vec<String> = sqlx::query(&format!("EXPLAIN QUERY PLAN {SQL}"))
            // The plan of the query for all of the library, which is the one
            // that goes through one thousand games: the parameter is NULL.
            .bind(None::<String>)
            .fetch_all(db.pool())
            .await
            .expect("plan")
            .iter()
            .map(|row| row.get::<String, _>("detail"))
            .collect();

        // To start at this index means to go, for each game, through all of the
        // copies of the library: `kind = 'owned'` removes nothing.
        let guilty: Vec<&String> = plan
            .iter()
            .filter(|step| step.contains("store_entry_by_kind"))
            .collect();

        assert!(
            guilty.is_empty(),
            "a subquery starts at store_entry again and not at game_link; \
             examine whether a CROSS JOIN was lost:\n{guilty:#?}"
        );
    }
}
