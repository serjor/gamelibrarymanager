use domain::{GameId, GamePrices};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::{game_id_from_text, game_id_to_text};
use crate::{Database, Result};

/// A wished-for game and the data necessary to ask for its price.
///
/// The ITAD identifier is present if it was resolved before. The Steam appid is
/// present when one copy of the game is from Steam, and it is what makes the
/// search exact: by title, ITAD decides alone and can be incorrect in the same
/// way as IGDB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceTarget {
    pub game_id: GameId,
    pub title: String,
    pub itad_id: Option<String>,
    pub steam_app_id: Option<String>,
}

/// The price of a game as it is shown: the least expensive offer at this
/// moment, how many stores sell it, and the lowest price that it has had.
///
/// The quantities are cents. They are formatted when they are shown and not
/// before: the currency goes with them, and to divide by one hundred here would
/// give back a number that you can no longer add without an error.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PriceRow {
    pub game_id: GameId,
    pub shop: String,
    pub amount: i64,
    pub regular: i64,
    pub cut: i64,
    pub currency: String,
    /// How many stores sell it at this moment.
    pub shops: i64,
    pub low_all_time: Option<i64>,
    pub low_year: Option<i64>,
    /// The name with which ITAD publishes the page of the game. It is the only
    /// address that the interface opens: the offer points to a store that can be
    /// any store, and the capability of the window cannot permit an unknown
    /// host.
    pub itad_slug: Option<String>,
    pub captured_at: i64,
}

pub struct PriceRepository<'a>(pub &'a Database);

impl PriceRepository<'_> {
    /// The wished-for games, with the data known about them to ask for the
    /// price.
    ///
    /// A game counts as wished for if it keeps **one or more** live copies of
    /// the `wishlist` kind. If the user also owns it in a different store, that
    /// does not remove it from this list: to have Disco Elysium in GOG and to
    /// want it in Steam is a real condition, and the user will know why.
    ///
    /// The `CROSS JOIN` clauses are those of the library query, and for the same
    /// reason: they make `game_link` control the plan, so that each subquery
    /// looks at the copies of its own game and does not go through the copies of
    /// all of the library. `MIN` on the appid is not a calculation, it breaks a
    /// tie: a game can have two Steam copies — the wished-for copy and the owned
    /// copy — and the search in ITAD must always use the same appid.
    pub async fn targets(&self) -> Result<Vec<PriceTarget>> {
        sqlx::query(
            "SELECT g.id, g.canonical_title, g.itad_id,
                    (SELECT MIN(e.store_app_id) FROM game_link l
                       CROSS JOIN store_entry e ON e.id = l.store_entry_id
                      WHERE l.game_id = g.id AND e.store = 'steam' AND e.deleted_at IS NULL
                    ) AS steam_app_id
             FROM game g
             WHERE g.deleted_at IS NULL
               AND EXISTS (
                   SELECT 1 FROM game_link l
                     CROSS JOIN store_entry e ON e.id = l.store_entry_id
                    WHERE l.game_id = g.id AND e.kind = 'wishlist' AND e.deleted_at IS NULL
               )
             ORDER BY g.sort_title",
        )
        .fetch_all(self.0.pool())
        .await?
        .iter()
        .map(|row| {
            Ok(PriceTarget {
                game_id: game_id_from_text(&row.get::<String, _>("id"))?,
                title: row.get("canonical_title"),
                itad_id: row.get("itad_id"),
                steam_app_id: row.get("steam_app_id"),
            })
        })
        .collect()
    }

    /// Replaces the prices of a game with the prices that have just come in.
    ///
    /// It replaces, it does not accumulate: the old rows are really deleted.
    /// This is the only exception in the schema to the logical delete, and the
    /// migration `0007_prices` gives the reason. An offer that ended and stays
    /// marked continues to look like an offer.
    pub async fn save(&self, game_id: GameId, prices: &GamePrices) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let id = game_id_to_text(game_id);
        let mut tx = self.0.pool().begin().await?;

        sqlx::query("DELETE FROM price_snapshot WHERE game_id = ?")
            .bind(&id)
            .execute(&mut *tx)
            .await?;

        for deal in &prices.deals {
            sqlx::query(
                "INSERT INTO price_snapshot
                     (game_id, shop, amount, regular, cut, currency, captured_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (game_id, shop) DO UPDATE SET
                     amount      = excluded.amount,
                     regular     = excluded.regular,
                     cut         = excluded.cut,
                     currency    = excluded.currency,
                     captured_at = excluded.captured_at",
            )
            .bind(&id)
            .bind(&deal.shop)
            .bind(deal.price.cents)
            .bind(deal.regular.cents)
            .bind(deal.cut)
            .bind(&deal.price.currency)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        // The currency of the low comes from the low itself, and if there is
        // no low, from the first offer: the two come from the same country that
        // was requested, thus they cannot disagree.
        let currency = prices
            .low_all_time
            .as_ref()
            .or(prices.low_year.as_ref())
            .map(|money| money.currency.clone())
            .or_else(|| prices.deals.first().map(|deal| deal.price.currency.clone()))
            .unwrap_or_default();

        sqlx::query(
            "INSERT INTO price_low (game_id, all_time, year, currency, captured_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (game_id) DO UPDATE SET
                 all_time    = excluded.all_time,
                 year        = excluded.year,
                 currency    = excluded.currency,
                 captured_at = excluded.captured_at",
        )
        .bind(&id)
        .bind(prices.low_all_time.as_ref().map(|money| money.cents))
        .bind(prices.low_year.as_ref().map(|money| money.cents))
        .bind(currency)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Forgets the prices of the games that are no longer in a wishlist.
    ///
    /// A game that the user buys leaves the list, and its price stops having a
    /// meaning. You give all of the wishlist and not only the games that were
    /// just refreshed, so that a cancel in the middle does not delete the data
    /// that is still applicable.
    pub async fn forget_missing(&self, keep: &[GameId]) -> Result<u64> {
        let placeholders = vec!["?"; keep.len()].join(",");
        let sql = format!("DELETE FROM price_snapshot WHERE game_id NOT IN ({placeholders})");
        let mut query = sqlx::query(&sql);
        for game_id in keep {
            query = query.bind(game_id_to_text(*game_id));
        }
        let deleted = query.execute(self.0.pool()).await?.rows_affected();

        let sql = format!("DELETE FROM price_low WHERE game_id NOT IN ({placeholders})");
        let mut query = sqlx::query(&sql);
        for game_id in keep {
            query = query.bind(game_id_to_text(*game_id));
        }
        query.execute(self.0.pool()).await?;

        Ok(deleted)
    }

    /// The best price of each game, in one query.
    ///
    /// For the same reason that all of the library comes from one query: the
    /// wishlist is shown complete, and one query for each game to find the least
    /// expensive store is the easiest way to make the display jump.
    pub async fn all(&self) -> Result<Vec<PriceRow>> {
        sqlx::query(SQL)
            .fetch_all(self.0.pool())
            .await?
            .iter()
            .map(hydrate)
            .collect()
    }
}

/// The least expensive offer of each game, with the two values that go with it.
///
/// `ROW_NUMBER` and not a `MIN(amount)` with independent columns: SQLite lets
/// you write that, but when two stores are equal to the cent it selects one at
/// random, and the store that is shown would change between two starts of the
/// application. With the tie broken by name, the same data always gives the
/// same row.
///
/// `price_snapshot` controls the result: a game that nobody sells at this moment
/// has no price to show, and its row does not exist. The all-time low comes with
/// the offer, never alone.
const SQL: &str = "SELECT best.game_id, best.shop, best.amount, best.regular, best.cut,
                          best.currency, best.captured_at, best.shops,
                          g.itad_slug,
                          l.all_time AS low_all_time,
                          l.year     AS low_year
                     FROM (
                          SELECT p.game_id, p.shop, p.amount, p.regular, p.cut, p.currency,
                                 p.captured_at,
                                 COUNT(*)     OVER (PARTITION BY p.game_id) AS shops,
                                 ROW_NUMBER() OVER (PARTITION BY p.game_id
                                                    ORDER BY p.amount, p.shop) AS ordinal
                            FROM price_snapshot p
                     ) best
                     JOIN game g ON g.id = best.game_id
                     LEFT JOIN price_low l ON l.game_id = best.game_id
                    WHERE best.ordinal = 1 AND g.deleted_at IS NULL";

fn hydrate(row: &SqliteRow) -> Result<PriceRow> {
    let captured: OffsetDateTime = row.get("captured_at");

    Ok(PriceRow {
        game_id: game_id_from_text(&row.get::<String, _>("game_id"))?,
        shop: row.get("shop"),
        amount: row.get("amount"),
        regular: row.get("regular"),
        cut: row.get("cut"),
        currency: row.get("currency"),
        shops: row.get("shops"),
        low_all_time: row.get("low_all_time"),
        low_year: row.get("low_year"),
        itad_slug: row.get("itad_slug"),
        captured_at: captured.unix_timestamp(),
    })
}
