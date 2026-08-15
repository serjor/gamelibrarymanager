//! What a game costs, said in a way that does not depend on who counts it.
//!
//! It lives here and not in the provider for the same reason as `Candidate`:
//! the code that asks for the prices and the code that keeps them do not know
//! each other, and the two must speak about the same thing.

use serde::{Deserialize, Serialize};

/// An applicable quantity of money, in the smallest unit of its currency.
///
/// Cents and not a number with decimals. A price is a count, and in floating
/// point 19.99 stops being 19.99 as soon as you add two of them: the error
/// shows when you compare against an all-time low, which is what this screen
/// does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub cents: i64,
    pub currency: String,
}

/// What a store asks today for a game.
///
/// `shop` is the name that the price provider gives it, and not a `StoreId`:
/// the stores that sell are many more than the three that this program can
/// read, and the best price of a wished-for game is very frequently in one of
/// the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deal {
    pub shop: String,
    pub price: Money,
    pub regular: Money,
    /// The discount as a percentage, as the provider gives it. It is not
    /// calculated again: two different roundings of the same discount disagree
    /// on the screen.
    pub cut: i64,
}

/// The prices of a game: what it costs now in each store and what it has cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamePrices {
    /// The identifier with which the price provider knows it.
    pub provider_id: String,
    /// The all-time low and the low of the last year. They are absent when the
    /// game has never been on offer, which is not the same as a cost of zero.
    pub low_all_time: Option<Money>,
    pub low_year: Option<Money>,
    pub deals: Vec<Deal>,
}
