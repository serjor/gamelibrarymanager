//! Caso de uso de identidad: convertir entradas de tienda en fichas de juego.
//!
//! El orden no es negociable. Primero el identificador externo, que es exacto;
//! solo cuando no lo hay se recurre al parecido de títulos, y ahí decide
//! `domain::matching`, que ante la duda manda a revisión.
//!
//! Escribe en `game`, `game_link` y `match_candidate`. Nunca en `store_entry`
//! —eso es de la tienda— ni en `user_state` —eso es del usuario—.

use domain::{Game, GameId, GameLink, LinkMethod, MatchDecision, StoreEntry, StoreId, matching};
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use serde::Serialize;
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, MatchCandidateRepository, StoreEntryRepository,
};

use crate::error::AppError;

#[derive(Debug, Default, Serialize)]
pub struct IdentityReport {
    /// Enlazadas sin preguntar.
    pub linked: usize,
    /// A la cola de revisión.
    pub review: usize,
    /// Sin ningún candidato: ni IGDB las conoce.
    pub unknown: usize,
}

pub async fn resolve(
    db: &Database,
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
) -> Result<IdentityReport, AppError> {
    let entries = StoreEntryRepository(db);
    // Las que nunca tuvieron ficha y las que tienen una hecha solo con el
    // título de la tienda: estas segundas ya se ven en la biblioteca, pero
    // siguen esperando una identidad de verdad.
    let mut pending = entries.unlinked().await?;
    pending.extend(entries.pending_metadata().await?);

    let mut report = IdentityReport::default();
    let mut links = GameLinkRepository(db).all().await?;

    for entry in pending {
        let decision = decide(igdb, credentials, token, &entry).await?;
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
                    ensure_game(db, igdb, credentials, token, igdb_id, &entry, ficha_local).await?;
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
    }

    // Un solo `rebuild_auto` al final: reescribe los enlaces automáticos de una
    // vez y respeta los manuales, que es la garantía de la fase 2.
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
pub async fn resolve_local(db: &Database) -> Result<IdentityReport, AppError> {
    let games = GameRepository(db);
    let mut report = IdentityReport::default();
    let mut links = GameLinkRepository(db).all().await?;

    for entry in StoreEntryRepository(db).unlinked().await? {
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

async fn decide(
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    entry: &StoreEntry,
) -> Result<MatchDecision, AppError> {
    // Steam publica su appid en `external_games`: es exacto y ahorra toda la
    // incertidumbre del parecido de títulos.
    if entry.store == StoreId::Steam
        && let Some(igdb_id) = igdb
            .by_steam_app_id(credentials, token, &entry.store_app_id)
            .await?
    {
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
