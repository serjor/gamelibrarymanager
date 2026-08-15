//! Caso de uso de precios: poner un precio a cada juego de la lista de deseados.
//!
//! Va aparte de la sincronización y no dentro de ella a propósito. Son cosas
//! distintas: sincronizar lee las tiendas del usuario y esto pregunta a un
//! tercero cuánto cuesta algo. Juntarlas haría que ITAD caído dejara sin
//! sincronizar Steam, que es justo lo que la fase 7 se dedicó a evitar.
//!
//! Escribe en `price_snapshot`, en `price_low` y en las dos columnas de `game`
//! que guardan el identificador de ITAD. Nunca en `store_entry` —eso es de la
//! tienda— ni en `user_state` —eso es del usuario—.
//!
//! Recibe sus colaboradores en lugar de sacarlos del estado global, como la
//! sincronización: así se prueba de extremo a extremo contra un servidor de
//! mentira y una base de datos de verdad, sin arrancar Tauri.

use std::collections::HashMap;

use domain::GameId;
use metadata::ItadClient;
use metadata::itad::ItadCredentials;
use serde::Serialize;
use storage::Database;
use storage::repositories::{GameRepository, PriceRepository, PriceTarget};

use crate::error::AppError;
use crate::sync::{ProgressSink, SyncProgress};

/// De dónde salen los precios. Es el nombre que se enseña, y también el que
/// viaja en el progreso.
const PROVEEDOR: &str = "itad";

#[derive(Debug, Default, Serialize)]
pub struct PriceReport {
    /// Deseados con al menos una tienda que los venda ahora mismo.
    pub priced: usize,
    /// Deseados que ITAD no sabe identificar. No es un error: hay juegos que no
    /// tiene, y la siguiente pasada vuelve a preguntar por ellos.
    pub unknown: usize,
    /// El usuario paró a mitad. Lo consultado se queda: es idempotente.
    pub cancelled: bool,
}

pub async fn refresh(
    db: &Database,
    itad: &ItadClient,
    credentials: &ItadCredentials,
    progress: &dyn ProgressSink,
) -> Result<PriceReport, AppError> {
    let prices = PriceRepository(db);
    let targets = prices.targets().await?;
    let mut report = PriceReport::default();

    // Lo que ya no está en ninguna lista de deseados deja de tener precio. Se
    // hace lo primero y con la lista completa —no con lo que se llegue a
    // consultar—, así que cancelar a mitad no borra nada que siga valiendo.
    let vivos: Vec<GameId> = targets.iter().map(|target| target.game_id).collect();
    prices.forget_missing(&vivos).await?;

    // Un identificador puede tocar a más de una ficha: dos fichas locales del
    // mismo juego, todavía sin unificar, resuelven al mismo juego de ITAD y las
    // dos tienen que acabar con su precio.
    let mut por_id: HashMap<String, Vec<GameId>> = HashMap::new();
    let total = targets.len();

    for (indice, target) in targets.into_iter().enumerate() {
        // Se para entre juegos, nunca a mitad de uno.
        if progress.cancelled() {
            report.cancelled = true;
            break;
        }
        progress.report(SyncProgress {
            store: PROVEEDOR.to_owned(),
            stage: "buscando en ITAD",
            done: indice,
            total,
        });

        match resolve(db, itad, credentials, &target).await? {
            Some(itad_id) => por_id.entry(itad_id).or_default().push(target.game_id),
            None => report.unknown += 1,
        }
    }

    if por_id.is_empty() {
        return Ok(report);
    }

    progress.report(SyncProgress {
        store: PROVEEDOR.to_owned(),
        stage: "precios",
        done: 0,
        total: por_id.len(),
    });

    // Una sola consulta por cada doscientos juegos: el cliente parte la lista.
    let ids: Vec<String> = por_id.keys().cloned().collect();
    for game_prices in itad.prices(credentials, &ids).await? {
        let Some(games) = por_id.get(&game_prices.provider_id) else {
            continue;
        };
        for game_id in games {
            prices.save(*game_id, &game_prices).await?;
            if !game_prices.deals.is_empty() {
                report.priced += 1;
            }
        }
    }

    Ok(report)
}

/// Con qué identificador conoce ITAD este juego.
///
/// El que ya se sabía, si lo hay. Si no, el appid de Steam, que es exacto, y en
/// último lugar el título, que es una apuesta. El resultado se anota en la
/// ficha: una lista de deseados es corta, pero preguntar lo mismo en cada
/// pasada gasta cuota para nada.
async fn resolve(
    db: &Database,
    itad: &ItadClient,
    credentials: &ItadCredentials,
    target: &PriceTarget,
) -> Result<Option<String>, AppError> {
    if let Some(known) = &target.itad_id {
        return Ok(Some(known.clone()));
    }

    let found = match &target.steam_app_id {
        Some(app_id) => itad.lookup_by_steam_app_id(credentials, app_id).await?,
        None => itad.lookup_by_title(credentials, &target.title).await?,
    };

    match found {
        Some(game) => {
            GameRepository(db)
                .set_itad(target.game_id, &game.id, &game.slug)
                .await?;
            Ok(Some(game.id))
        }
        None => Ok(None),
    }
}
