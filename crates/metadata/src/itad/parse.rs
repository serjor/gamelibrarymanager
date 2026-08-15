//! Lectura de las respuestas de ITAD, separada del transporte para poder
//! probarla con respuestas grabadas y sin red.

use domain::{Deal, GamePrices, Money};
use serde::Deserialize;

use super::ItadGame;
use crate::{MetadataError, Result};

#[derive(Deserialize)]
struct LookupResponse {
    found: bool,
    #[serde(default)]
    game: Option<RawGame>,
}

#[derive(Deserialize)]
struct RawGame {
    id: String,
    slug: String,
    title: String,
}

#[derive(Deserialize)]
struct RawPrices {
    id: String,
    #[serde(rename = "historyLow", default)]
    history_low: Option<RawHistoryLow>,
    #[serde(default)]
    deals: Vec<RawDeal>,
}

#[derive(Deserialize)]
struct RawHistoryLow {
    #[serde(default)]
    all: Option<RawPrice>,
    #[serde(default)]
    y1: Option<RawPrice>,
}

#[derive(Deserialize)]
struct RawDeal {
    shop: RawShop,
    price: RawPrice,
    regular: RawPrice,
    cut: i64,
}

#[derive(Deserialize)]
struct RawShop {
    name: String,
}

/// El precio tal y como llega: con el importe repetido en dos formas.
///
/// Se lee `amountInt`, que son céntimos enteros, y se ignora `amount`, que es el
/// mismo número en coma flotante. La forma que no pierde nada es la entera.
#[derive(Deserialize)]
struct RawPrice {
    #[serde(rename = "amountInt")]
    amount_int: i64,
    currency: String,
}

impl From<RawPrice> for Money {
    fn from(price: RawPrice) -> Self {
        Self {
            cents: price.amount_int,
            currency: price.currency,
        }
    }
}

pub fn parse_lookup(body: &str) -> Result<Option<ItadGame>> {
    let parsed: LookupResponse = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("respuesta ilegible: {e}")))?;

    // `found` y el juego son dos cosas distintas en la respuesta, así que se
    // exigen las dos: un `found: true` sin juego no es un juego encontrado.
    if !parsed.found {
        return Ok(None);
    }

    Ok(parsed.game.map(|game| ItadGame {
        id: game.id,
        slug: game.slug,
        title: game.title,
    }))
}

pub fn parse_prices(body: &str) -> Result<Vec<GamePrices>> {
    let parsed: Vec<RawPrices> = serde_json::from_str(body)
        .map_err(|e| MetadataError::Unexpected(format!("respuesta ilegible: {e}")))?;

    Ok(parsed
        .into_iter()
        .map(|game| {
            let (low_all_time, low_year) = match game.history_low {
                Some(low) => (low.all.map(Money::from), low.y1.map(Money::from)),
                None => (None, None),
            };
            GamePrices {
                provider_id: game.id,
                low_all_time,
                low_year,
                deals: game
                    .deals
                    .into_iter()
                    .map(|deal| Deal {
                        shop: deal.shop.name,
                        price: deal.price.into(),
                        regular: deal.regular.into(),
                        cut: deal.cut,
                    })
                    .collect(),
            }
        })
        .collect())
}
