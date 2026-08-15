//! Caso de uso de identidad: convertir entradas de tienda en fichas de juego.
//!
//! El orden no es negociable. Primero el identificador externo, que es exacto;
//! solo cuando no lo hay se recurre al parecido de títulos, y ahí decide
//! `domain::matching`, que ante la duda manda a revisión.
//!
//! Las tres tiendas tienen identificador externo, y cada una el suyo: el appid
//! de Steam, el `external_id` de Galaxy y la oferta de Epic. Los cruces se piden
//! **todos de golpe antes del bucle**, en lotes de 500, y no copia a copia. A 4
//! peticiones por segundo, una biblioteca de 1.200 copias tardaba cinco minutos
//! en cruzarse y el usuario cancelaba antes del final; lo que se quedaba sin
//! cruzar caía en la búsqueda por título, que es justo la vía dudosa que el
//! identificador existe para evitar.
//!
//! Escribe en `game`, `game_link` y `match_candidate`. Nunca en `store_entry`
//! —eso es de la tienda— ni en `user_state` —eso es del usuario—.

use std::collections::HashMap;

use domain::{
    Game, GameId, GameLink, LinkMethod, MatchDecision, StoreEntry, StoreEntryId, StoreId, matching,
};
use metadata::IgdbClient;
use metadata::igdb::{ExternalSource, IgdbCredentials, IgdbToken};
use serde::Serialize;
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, MatchCandidateRepository, StoreEntryRepository,
};

use crate::error::AppError;
use crate::sync::{ProgressSink, SyncProgress};

#[derive(Debug, Default, Serialize)]
pub struct IdentityReport {
    /// Enlazadas sin preguntar.
    pub linked: usize,
    /// A la cola de revisión.
    pub review: usize,
    /// Sin ningún candidato: ni IGDB las conoce.
    pub unknown: usize,
    /// El usuario paró a mitad. Lo emparejado se queda: es idempotente.
    pub cancelled: bool,
    /// El proveedor cortó y la pasada se detuvo ahí, con el motivo.
    ///
    /// Es un resultado, no un error: lo emparejado hasta ese punto está
    /// guardado y la siguiente pasada sigue por donde iba. Sube como error solo
    /// lo que no deja seguir, que es un fallo de la base de datos.
    pub stopped: Option<String>,
}

/// Cada cuántos juegos se guarda lo que se lleva emparejado.
///
/// Antes se escribía una sola vez, al final. Con mil juegos eso son minutos
/// —IGDB admite cuatro peticiones por segundo—, y un 429 en el juego trescientos
/// tiraba la pasada entera: ni un enlace escrito, y a empezar. Veinticinco
/// juegos son unos diez segundos de trabajo, que es lo que se puede perder
/// ahora.
///
/// Guardar de más no rompe nada: `rebuild_auto` reescribe el mismo conjunto de
/// enlaces cada vez, así que llamarlo veinte veces deja lo mismo que llamarlo
/// una.
const TRAMO: usize = 25;

pub async fn resolve(
    db: &Database,
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    progress: &dyn ProgressSink,
) -> Result<IdentityReport, AppError> {
    let entries = StoreEntryRepository(db);
    // Las que nunca tuvieron ficha y las que tienen una hecha solo con el
    // título de la tienda: estas segundas ya se ven en la biblioteca, pero
    // siguen esperando una identidad de verdad.
    let mut pending = entries.unlinked().await?;
    pending.extend(entries.pending_metadata().await?);

    let mut report = IdentityReport::default();
    let mut links = GameLinkRepository(db).all().await?;
    let total = pending.len();
    let mut desde_el_ultimo_guardado = 0;

    progress.report(SyncProgress {
        store: "igdb".to_owned(),
        stage: "cruzando identificadores",
        done: 0,
        total,
    });
    // El cruce va antes del bucle, así que un corte aquí no deja nada a medias:
    // es el tramo cero. Se dice por qué se para y no se empareja nada, en vez de
    // dejar caer la biblioteca entera en la búsqueda por título, que enlazaría
    // peor de lo que enlaza el identificador.
    let external = match external_ids(igdb, credentials, token, &pending).await {
        Ok(external) => external,
        Err(AppError::Metadata(error)) => {
            report.stopped = Some(error.to_string());
            return Ok(report);
        }
        Err(otro) => return Err(otro),
    };

    for (indice, entry) in pending.into_iter().enumerate() {
        // Se para entre juegos, nunca a mitad de uno. Lo ya decidido se
        // conserva y la siguiente pasada sigue por donde iba.
        if progress.cancelled() {
            report.cancelled = true;
            break;
        }
        progress.report(SyncProgress {
            store: entry.store.as_str().to_owned(),
            stage: "emparejando",
            done: indice,
            total,
        });

        let decision = match decide(
            igdb,
            credentials,
            token,
            &entry,
            external.get(&entry.id).copied(),
        )
        .await
        {
            Ok(decision) => decision,
            // Un corte del proveedor para la pasada aquí mismo, y lo de atrás se
            // guarda igual. Un fallo de la base de datos sí sube: si no se puede
            // escribir, no hay nada que salvar.
            Err(AppError::Metadata(error)) => {
                report.stopped = Some(error.to_string());
                break;
            }
            Err(otro) => return Err(otro),
        };
        let ficha_local = links
            .iter()
            .find(|link| link.store_entry_id == entry.id)
            .map(|link| link.game_id);

        match decision {
            MatchDecision::Auto {
                igdb_id,
                confidence,
            } => {
                let game_id =
                    match ensure_game(db, igdb, credentials, token, igdb_id, &entry, ficha_local)
                        .await
                    {
                        Ok(game_id) => game_id,
                        Err(AppError::Metadata(error)) => {
                            report.stopped = Some(error.to_string());
                            break;
                        }
                        Err(otro) => return Err(otro),
                    };
                // La entrada puede traer ya un enlace local: se sustituye, no se
                // acumula. Con dos propuestas para la misma entrada, el índice
                // único decidiría por orden de inserción cuál gana.
                links.retain(|link| link.store_entry_id != entry.id);
                links.push(GameLink {
                    game_id,
                    store_entry_id: entry.id,
                    confidence,
                    method: LinkMethod::Auto,
                });
                MatchCandidateRepository(db).clear(entry.id).await?;
                report.linked += 1;
            }
            // Sin decisión, el enlace local que hubiera se queda como estaba: ya
            // está en `links` y `rebuild_auto` lo reescribirá igual. Quitarlo
            // haría desaparecer de la biblioteca un juego que el usuario ya veía.
            MatchDecision::Review { candidates } => {
                if candidates.is_empty() {
                    report.unknown += 1;
                } else {
                    report.review += 1;
                }
                MatchCandidateRepository(db)
                    .replace(entry.id, &candidates)
                    .await?;
            }
        }

        desde_el_ultimo_guardado += 1;
        if desde_el_ultimo_guardado == TRAMO {
            GameLinkRepository(db).rebuild_auto(&links).await?;
            desde_el_ultimo_guardado = 0;
        }
    }

    // Y el último tramo, que casi nunca cae justo en el corte. `rebuild_auto`
    // reescribe los enlaces automáticos de una vez y respeta los manuales, que
    // es la garantía de la fase 2.
    GameLinkRepository(db).rebuild_auto(&links).await?;
    GameRepository(db).soft_delete_orphans().await?;
    Ok(report)
}

/// Emparejamiento sin IGDB: agrupa las copias por título normalizado y les crea
/// una ficha con lo que dice la tienda.
///
/// Existe porque bloquear la aplicación entera hasta que el usuario consiga unas
/// credenciales de Twitch es muy duro en el primer arranque. Lo que sale de aquí
/// es una biblioteca de verdad —con su estado y sus insignias de tienda— a la
/// espera de metadatos, y el mismo título en dos tiendas ya cae en una sola
/// ficha: para eso basta la normalización, IGDB solo añade la certeza.
pub async fn resolve_local(
    db: &Database,
    progress: &dyn ProgressSink,
) -> Result<IdentityReport, AppError> {
    let games = GameRepository(db);
    let mut report = IdentityReport::default();
    let mut links = GameLinkRepository(db).all().await?;

    let pending = StoreEntryRepository(db).unlinked().await?;
    let total = pending.len();

    for (indice, entry) in pending.into_iter().enumerate() {
        if progress.cancelled() {
            report.cancelled = true;
            break;
        }
        progress.report(SyncProgress {
            store: entry.store.as_str().to_owned(),
            stage: "agrupando por título",
            done: indice,
            total,
        });

        let sort_title = matching::normalize(&entry.title);
        let game_id = match games.find_local_by_sort_title(&sort_title).await? {
            Some(existing) => existing.id,
            None => {
                let game = local_game(&entry);
                games.upsert(&game).await?;
                game.id
            }
        };

        links.retain(|link| link.store_entry_id != entry.id);
        links.push(GameLink {
            game_id,
            store_entry_id: entry.id,
            confidence: matching::LOCAL_TITLE_CONFIDENCE,
            method: LinkMethod::Auto,
        });
        report.linked += 1;
    }

    GameLinkRepository(db).rebuild_auto(&links).await?;
    Ok(report)
}

/// Cruza contra `external_games` todo lo que traiga identificador, tienda por
/// tienda y en lotes.
///
/// Lo que no cruce no aparece en el mapa, y eso es lo normal: las claves de
/// Amazon que GOG regala, las bandas sonoras y los prólogos no tienen ficha en
/// IGDB, y son la mayor parte de lo que falla. Esas copias siguen su camino por
/// el título.
async fn external_ids(
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    pending: &[StoreEntry],
) -> Result<HashMap<StoreEntryId, i64>, AppError> {
    const FUENTES: [(StoreId, ExternalSource); 3] = [
        (StoreId::Steam, ExternalSource::Steam),
        (StoreId::Gog, ExternalSource::Gog),
        (StoreId::Epic, ExternalSource::Epic),
    ];

    let mut resueltos = HashMap::new();

    for (store, source) in FUENTES {
        let de_la_tienda: Vec<(StoreEntryId, String)> = pending
            .iter()
            .filter(|entry| entry.store == store)
            .filter_map(|entry| external_uid(entry).map(|uid| (entry.id, uid)))
            .collect();
        if de_la_tienda.is_empty() {
            continue;
        }

        // El mismo juego puede estar en dos cuentas de la misma tienda, y
        // preguntar dos veces por él gastaría hueco del lote.
        let mut uids: Vec<String> = de_la_tienda.iter().map(|(_, uid)| uid.clone()).collect();
        uids.sort_unstable();
        uids.dedup();

        let cruces = igdb
            .by_external_ids(credentials, token, source, &uids)
            .await?;
        for (id, uid) in de_la_tienda {
            if let Some(igdb_id) = cruces.get(&uid) {
                resueltos.insert(id, *igdb_id);
            }
        }
    }

    Ok(resueltos)
}

/// El identificador con el que cada tienda aparece en `external_games`.
///
/// Steam y GOG publican el suyo en la propia copia. Epic no: lo que IGDB indexa
/// es la **oferta** de su tienda, que no viaja en el asset del lanzador, y por
/// eso el conector la resuelve durante la sincronización y la deja en `raw`.
/// Una copia de Epic sincronizada antes de que existiera ese campo no lo tiene
/// y se empareja por título hasta la siguiente pasada.
fn external_uid(entry: &StoreEntry) -> Option<String> {
    match entry.store {
        StoreId::Steam | StoreId::Gog => Some(entry.store_app_id.clone()),
        StoreId::Epic => entry
            .raw
            .get("offerId")
            .and_then(|offer| offer.as_str())
            .map(str::to_owned),
    }
}

async fn decide(
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    entry: &StoreEntry,
    external: Option<i64>,
) -> Result<MatchDecision, AppError> {
    // El identificador de la tienda es exacto y ahorra toda la incertidumbre
    // del parecido de títulos.
    if let Some(igdb_id) = external {
        return Ok(matching::decide_by_external_id(igdb_id));
    }

    let candidates = igdb.search(credentials, token, &entry.title).await?;
    Ok(matching::decide_by_title(&entry.title, None, &candidates))
}

/// Crea la ficha si no existe. La tabla `game` es también la caché de IGDB: si
/// el juego ya está, no se vuelve a preguntar nunca.
///
/// `ficha_local` es la ficha sin metadatos de la que ya colgaba esta copia, si
/// la había. Se **reutiliza su identificador** en vez de crear otra, y esa es
/// toda la diferencia: `user_state` cuelga del `game_id`, así que crear una
/// ficha nueva dejaría huérfano el estado que el usuario ya había escrito.
async fn ensure_game(
    db: &Database,
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    igdb_id: i64,
    entry: &StoreEntry,
    ficha_local: Option<GameId>,
) -> Result<GameId, AppError> {
    let games = GameRepository(db);
    if let Some(existing) = games.find_by_igdb(igdb_id).await? {
        return Ok(existing.id);
    }

    // Sin ficha previa se crea una; con ella se reescribe la que ya existía.
    // `GameId::default()` es `GameId::new()`, con su UUIDv7 recién hecho.
    let id = ficha_local.unwrap_or_default();
    let fetched = igdb.game(credentials, token, igdb_id).await?;
    let game = match fetched {
        Some(meta) => Game {
            id,
            canonical_title: meta.name.clone(),
            sort_title: matching::normalize(&meta.name),
            igdb_id: Some(meta.igdb_id),
            cover_url: meta.cover_url,
            summary: meta.summary,
            released_at: meta.released_at,
            genres: meta.genres,
        },
        // IGDB conoce el identificador pero no devuelve la ficha: mejor una
        // ficha con el título de la tienda que ninguna.
        None => Game {
            id,
            ..local_game(entry)
        },
    };

    games.upsert(&game).await?;
    Ok(game.id)
}

/// Ficha sin metadatos, construida con lo que dice la tienda. Es lo que se crea
/// cuando el usuario declara que un juego no está en IGDB.
pub fn local_game(entry: &StoreEntry) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: entry.title.clone(),
        sort_title: matching::normalize(&entry.title),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    }
}
