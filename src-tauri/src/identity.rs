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
    let pending = StoreEntryRepository(db).unlinked().await?;
    let mut report = IdentityReport::default();
    let mut links = GameLinkRepository(db).all().await?;

    for entry in pending {
        let decision = decide(igdb, credentials, token, &entry).await?;

        match decision {
            MatchDecision::Auto {
                igdb_id,
                confidence,
            } => {
                let game_id = ensure_game(db, igdb, credentials, token, igdb_id, &entry).await?;
                links.push(GameLink {
                    game_id,
                    store_entry_id: entry.id,
                    confidence,
                    method: LinkMethod::Auto,
                });
                MatchCandidateRepository(db).clear(entry.id).await?;
                report.linked += 1;
            }
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
async fn ensure_game(
    db: &Database,
    igdb: &IgdbClient,
    credentials: &IgdbCredentials,
    token: &IgdbToken,
    igdb_id: i64,
    entry: &StoreEntry,
) -> Result<GameId, AppError> {
    let games = GameRepository(db);
    if let Some(existing) = games.find_by_igdb(igdb_id).await? {
        return Ok(existing.id);
    }

    let fetched = igdb.game(credentials, token, igdb_id).await?;
    let game = match fetched {
        Some(meta) => Game {
            id: GameId::new(),
            canonical_title: meta.name.clone(),
            sort_title: matching::normalize(&meta.name),
            igdb_id: Some(meta.igdb_id),
            cover_url: meta.cover_url,
            summary: meta.summary,
            released_at: meta.released_at,
        },
        // IGDB conoce el identificador pero no devuelve la ficha: mejor una
        // ficha con el título de la tienda que ninguna.
        None => local_game(entry),
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
    }
}
