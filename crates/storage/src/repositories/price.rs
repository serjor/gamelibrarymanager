use domain::{GameId, GamePrices};
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use time::OffsetDateTime;

use crate::mapping::{game_id_from_text, game_id_to_text};
use crate::{Database, Result};

/// Un juego deseado y lo que hace falta para preguntar por su precio.
///
/// El identificador de ITAD viene si ya se resolvió alguna vez. El appid de
/// Steam viene cuando alguna copia del juego es de Steam, y es lo que convierte
/// la búsqueda en exacta: por título, ITAD decide por su cuenta y puede
/// equivocarse igual que se equivoca IGDB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceTarget {
    pub game_id: GameId,
    pub title: String,
    pub itad_id: Option<String>,
    pub steam_app_id: Option<String>,
}

/// El precio de un juego tal y como se enseña: la oferta más barata de ahora
/// mismo, cuántas tiendas lo venden, y hasta dónde ha llegado a bajar.
///
/// Los importes son céntimos. Se formatean al pintarlos y no antes: la moneda
/// viaja al lado, y dividir entre cien aquí sería devolver un número que ya no
/// se puede sumar sin error.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PriceRow {
    pub game_id: GameId,
    pub shop: String,
    pub amount: i64,
    pub regular: i64,
    pub cut: i64,
    pub currency: String,
    /// Cuántas tiendas lo venden ahora mismo.
    pub shops: i64,
    pub low_all_time: Option<i64>,
    pub low_year: Option<i64>,
    /// Con qué nombre publica ITAD la página del juego. Es la única dirección
    /// que la interfaz abre: la oferta apunta a la tienda que sea, y la
    /// capacidad de la ventana no puede permitir un host que no conoce.
    pub itad_slug: Option<String>,
    pub captured_at: i64,
}

pub struct PriceRepository<'a>(pub &'a Database);

impl PriceRepository<'_> {
    /// Los juegos deseados, con lo que se sabe de ellos para preguntar el
    /// precio.
    ///
    /// Un juego cuenta como deseado si le queda **alguna** copia de tipo
    /// `wishlist` viva. Que además se posea en otra tienda no lo saca de aquí:
    /// tener Disco Elysium en GOG y quererlo en Steam es una situación real, y
    /// el usuario sabrá por qué.
    ///
    /// Los `CROSS JOIN` son los de la consulta de la biblioteca y por el mismo
    /// motivo: fijan que mande `game_link`, para que cada subconsulta mire las
    /// copias de su juego en vez de recorrer las de la biblioteca entera.
    /// `MIN` sobre el appid no es un cálculo, es un desempate: un juego puede
    /// tener dos copias de Steam —la deseada y la que se posee— y la búsqueda
    /// en ITAD tiene que salir siempre con el mismo appid.
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

    /// Sustituye los precios de un juego por los que acaban de llegar.
    ///
    /// Sustituye, no acumula: las filas viejas se borran de verdad. Es la única
    /// excepción del esquema a la baja lógica, y está razonada en la migración
    /// `0007_prices`. Una oferta que terminó y se queda marcada sigue pareciendo
    /// una oferta.
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

        // La moneda del mínimo sale de la del propio mínimo, y si no hay
        // ninguno, de la de la primera oferta: los dos vienen del mismo país que
        // se pidió, así que no pueden discrepar.
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

    /// Olvida los precios de lo que ya no está en ninguna lista de deseados.
    ///
    /// Un juego que se compra sale de la lista, y su precio deja de significar
    /// nada. Se pasa la lista entera de deseados y no los que se acaban de
    /// refrescar, para que cancelar a mitad no borre lo que todavía vale.
    pub async fn forget_missing(&self, keep: &[GameId]) -> Result<u64> {
        let placeholders = vec!["?"; keep.len()].join(",");
        let sql = format!("DELETE FROM price_snapshot WHERE game_id NOT IN ({placeholders})");
        let mut query = sqlx::query(&sql);
        for game_id in keep {
            query = query.bind(game_id_to_text(*game_id));
        }
        let borradas = query.execute(self.0.pool()).await?.rows_affected();

        let sql = format!("DELETE FROM price_low WHERE game_id NOT IN ({placeholders})");
        let mut query = sqlx::query(&sql);
        for game_id in keep {
            query = query.bind(game_id_to_text(*game_id));
        }
        query.execute(self.0.pool()).await?;

        Ok(borradas)
    }

    /// El mejor precio de cada juego, en una consulta.
    ///
    /// Por el mismo motivo que la biblioteca entera sale de una sola: la lista
    /// de deseados se pinta entera, y una consulta por juego para saber cuál es
    /// la tienda más barata es la forma más fácil de que dé tirones.
    pub async fn all(&self) -> Result<Vec<PriceRow>> {
        sqlx::query(SQL)
            .fetch_all(self.0.pool())
            .await?
            .iter()
            .map(hydrate)
            .collect()
    }
}

/// La oferta más barata de cada juego, con las dos que la acompañan.
///
/// `ROW_NUMBER` y no un `MIN(amount)` con columnas sueltas: SQLite deja escribir
/// eso, pero cuando dos tiendas empatan al céntimo elige una cualquiera, y la
/// tienda que se enseña cambiaría entre dos aperturas de la aplicación. Con el
/// desempate por nombre, el mismo dato da siempre la misma fila.
///
/// Manda `price_snapshot`: un juego que ahora mismo no vende nadie no tiene
/// precio que enseñar, y su fila no existe. El mínimo histórico llega al lado,
/// nunca solo.
const SQL: &str = "SELECT mejor.game_id, mejor.shop, mejor.amount, mejor.regular, mejor.cut,
                          mejor.currency, mejor.captured_at, mejor.shops,
                          g.itad_slug,
                          l.all_time AS low_all_time,
                          l.year     AS low_year
                     FROM (
                          SELECT p.game_id, p.shop, p.amount, p.regular, p.cut, p.currency,
                                 p.captured_at,
                                 COUNT(*)     OVER (PARTITION BY p.game_id) AS shops,
                                 ROW_NUMBER() OVER (PARTITION BY p.game_id
                                                    ORDER BY p.amount, p.shop) AS puesto
                            FROM price_snapshot p
                     ) mejor
                     JOIN game g ON g.id = mejor.game_id
                     LEFT JOIN price_low l ON l.game_id = mejor.game_id
                    WHERE mejor.puesto = 1 AND g.deleted_at IS NULL";

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
