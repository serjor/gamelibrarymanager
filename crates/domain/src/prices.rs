//! Lo que cuesta un juego, dicho de una forma que no depende de quién lo
//! cuente.
//!
//! Vive aquí y no en el proveedor por lo mismo que `Candidate`: el que consulta
//! los precios y el que los guarda no se conocen entre ellos, y los dos
//! necesitan hablar de lo mismo.

use serde::{Deserialize, Serialize};

/// Una cantidad de dinero, en la unidad más pequeña de su moneda.
///
/// Céntimos y no un número con decimales. Un precio es un recuento, y en coma
/// flotante 19,99 deja de valer 19,99 en cuanto se suman dos: el error aparece
/// al comparar contra un mínimo histórico, que es justo lo que hace esta
/// pantalla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub cents: i64,
    pub currency: String,
}

/// Lo que una tienda pide hoy por un juego.
///
/// `shop` es el nombre que le da el proveedor de precios, y no una `StoreId`:
/// las tiendas que venden son muchas más que las tres que este programa sabe
/// leer, y el mejor precio de un deseado vive muy a menudo en una de las otras.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deal {
    pub shop: String,
    pub price: Money,
    pub regular: Money,
    /// Descuento en porcentaje, tal y como lo da el proveedor. No se recalcula:
    /// dos redondeos distintos del mismo descuento se contradicen en pantalla.
    pub cut: i64,
}

/// Los precios de un juego: lo que cuesta ahora en cada tienda y lo que ha
/// llegado a costar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamePrices {
    /// Con qué identificador lo conoce el proveedor de precios.
    pub provider_id: String,
    /// Mínimo histórico y mínimo del último año. Faltan cuando el juego nunca
    /// ha estado de oferta, que no es lo mismo que costar cero.
    pub low_all_time: Option<Money>,
    pub low_year: Option<Money>,
    pub deals: Vec<Deal>,
}
