//! Comandos expuestos a la UI. Orquestan casos de uso y traducen tipos: la
//! lógica vive en los crates de dominio y adaptadores.

// Público porque `generate_handler!` necesita llegar al elemento que genera
// `#[tauri::command]`, y un `pub use` de la función sola no lo arrastra.
pub mod gog;

use domain::{
    AuthContext, EntryKind, GameId, GameLink, LinkMethod, PlayStatus, ScoredCandidate,
    StoreAccount, StoreAccountId, StoreEntryId, StoreId, UserState,
};
use metadata::igdb::{IgdbCredentials, IgdbToken};
use serde::Serialize;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, LibraryRow, MatchCandidateRepository,
    StoreAccountRepository, StoreEntryRepository, UserStateRepository,
};
use tauri::{AppHandle, Emitter, State};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;
use crate::identity::{self, IdentityReport};
use crate::state::{AppState, IGDB_CREDENTIALS, IGDB_TOKEN, credential_key};
use crate::sync::{self, ProgressSink, SyncProgress, SyncReport};

#[derive(Serialize)]
pub struct AppInfo {
    pub version: &'static str,
    /// `keyring` o `passphrase`: decide si la UI tiene que pedir contraseña.
    pub secrets_backend: secrets::Backend,
    pub unlocked: bool,
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> Result<AppInfo, AppError> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        secrets_backend: state.backend,
        unlocked: state.is_unlocked().await,
    })
}

/// Abre el almacén cifrado en las máquinas sin keyring.
#[tauri::command]
pub async fn unlock_secrets(
    state: State<'_, AppState>,
    passphrase: String,
) -> Result<(), AppError> {
    state.unlock(&passphrase).await?;
    Ok(())
}

/// Conecta una cuenta de Steam validando la clave contra la API antes de
/// guardarla: así un error de copiar y pegar se ve al momento y no como una
/// biblioteca vacía.
#[tauri::command]
pub async fn connect_steam(
    state: State<'_, AppState>,
    api_key: String,
    steam_id: String,
) -> Result<StoreAccountId, AppError> {
    let connector = state
        .connectors
        .get(&StoreId::Steam)
        .ok_or_else(|| AppError::Message("sin conector de Steam".to_owned()))?;

    let session = connector
        .authenticate(&AuthContext::ApiKey {
            key: api_key.trim().to_owned(),
            account_ref: steam_id.trim().to_owned(),
        })
        .await?;

    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: session.account_ref.clone(),
        display_name: session.display_name.clone(),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    let id = StoreAccountRepository(&state.db).upsert(&account).await?;

    // La credencial va al almacén de secretos. La base de datos solo sabe que
    // la cuenta existe.
    state
        .secrets()
        .await?
        .set(&credential_key(&account), &session.credential)?;

    Ok(id)
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<AccountView>, AppError> {
    Ok(StoreAccountRepository(&state.db)
        .active()
        .await?
        .into_iter()
        .map(|account| AccountView {
            store: account.store.as_str(),
            account_ref: account.account_ref,
            display_name: account.display_name,
            last_sync_at: account.last_sync_at.map(|t| t.unix_timestamp()),
        })
        .collect())
}

#[derive(Serialize)]
pub struct AccountView {
    pub store: &'static str,
    pub account_ref: String,
    pub display_name: Option<String>,
    pub last_sync_at: Option<i64>,
}

/// Emite el progreso a la ventana y consulta la bandera de cancelación.
struct WindowProgress<'a> {
    app: AppHandle,
    state: &'a AppState,
}

impl ProgressSink for WindowProgress<'_> {
    fn report(&self, progress: SyncProgress) {
        // Si la ventana ya no está, el progreso da igual: no es motivo para
        // abortar una operación que por lo demás va bien.
        let _ = self.app.emit("sync:progress", progress);
    }

    fn cancelled(&self) -> bool {
        self.state.operation_cancelled()
    }
}

/// La sincronización corre en el runtime de Tauri y va emitiendo progreso, así
/// que la ventana sigue respondiendo mientras dura.
#[tauri::command]
pub async fn sync_now(app: AppHandle, state: State<'_, AppState>) -> Result<SyncReport, AppError> {
    state.begin_operation();
    let progress = WindowProgress {
        app: app.clone(),
        state: &state,
    };
    let report = sync::sync_all(&state, &progress).await;
    state.end_operation();
    report
}

/// Vale tanto para la sincronización como para el emparejamiento: los dos son
/// largos, los dos se paran en el siguiente punto seguro y nunca corren a la vez.
#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>) {
    state.cancel_operation();
}

/// La biblioteca entera en una consulta. Con mil juegos, hacer una consulta por
/// juego para saber en qué tiendas está es lo que hace que la rejilla dé
/// tirones.
#[tauri::command]
pub async fn library(state: State<'_, AppState>) -> Result<Vec<LibraryRow>, AppError> {
    Ok(LibraryRepository(&state.db).all().await?)
}

/// Lo único que escribe el usuario. Ninguna sincronización posterior lo toca.
#[tauri::command]
pub async fn set_user_state(
    state: State<'_, AppState>,
    game_id: String,
    status: Option<PlayStatus>,
    rating: Option<u8>,
    notes: Option<String>,
) -> Result<(), AppError> {
    let game_id = Uuid::parse_str(&game_id)
        .map(GameId::from_uuid)
        .map_err(|_| AppError::Message("identificador de juego inválido".to_owned()))?;

    let previous = UserStateRepository(&state.db).find(game_id).await?;
    UserStateRepository(&state.db)
        .save(&UserState {
            game_id,
            status,
            rating,
            notes,
            started_at: previous.as_ref().and_then(|p| p.started_at),
            finished_at: previous.as_ref().and_then(|p| p.finished_at),
        })
        .await?;
    Ok(())
}

#[derive(Serialize)]
pub struct LibrarySummary {
    pub owned: usize,
    pub wishlist: usize,
    pub games: usize,
    pub pending_review: usize,
}

#[tauri::command]
pub async fn library_summary(state: State<'_, AppState>) -> Result<LibrarySummary, AppError> {
    let entries = StoreEntryRepository(&state.db);
    Ok(LibrarySummary {
        owned: entries.active(EntryKind::Owned).await?.len(),
        wishlist: entries.active(EntryKind::Wishlist).await?.len(),
        games: GameRepository(&state.db).all().await?.len(),
        pending_review: entries.unlinked().await?.len(),
    })
}

/// Guarda las credenciales de IGDB del usuario, comprobándolas antes: si el
/// client secret está mal, se sabe aquí y no en mitad de la primera
/// sincronización.
#[tauri::command]
pub async fn set_igdb_credentials(
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> Result<(), AppError> {
    let credentials = IgdbCredentials {
        client_id: client_id.trim().to_owned(),
        client_secret: client_secret.trim().to_owned(),
    };
    let token = state.igdb.token(&credentials).await?;

    let secrets = state.secrets().await?;
    secrets.set(IGDB_CREDENTIALS, &serde_json::to_string(&credentials)?)?;
    secrets.set(IGDB_TOKEN, &serde_json::to_string(&token)?)?;
    Ok(())
}

#[tauri::command]
pub async fn has_igdb_credentials(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(state.secrets().await?.get(IGDB_CREDENTIALS)?.is_some())
}

/// Empareja lo que haya llegado de las tiendas con las fichas de IGDB.
///
/// Sin credenciales de IGDB no se para: se agrupan las copias por título y se
/// les crea una ficha con lo que dice la tienda. La biblioteca se puede usar
/// desde el primer arranque, y el día que el usuario configure IGDB estas fichas
/// se enriquecen en su sitio sin perder lo que haya escrito encima.
#[tauri::command]
pub async fn resolve_identities(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<IdentityReport, AppError> {
    state.begin_operation();
    let progress = WindowProgress {
        app: app.clone(),
        state: &state,
    };

    // El emparejamiento es lento por el límite de 4 peticiones por segundo de
    // IGDB, así que va informando juego a juego en vez de dejar la ventana
    // callada varios minutos.
    let report = match igdb_session(&state).await {
        Ok((credentials, token)) => {
            identity::resolve(&state.db, &state.igdb, &credentials, &token, &progress).await
        }
        Err(AppError::MissingIgdbCredentials) => {
            identity::resolve_local(&state.db, &progress).await
        }
        Err(other) => Err(other),
    };

    state.end_operation();
    report
}

/// El token de Twitch dura unos sesenta días: se guarda y solo se renueva
/// cuando caduca de verdad.
async fn igdb_session(state: &AppState) -> Result<(IgdbCredentials, IgdbToken), AppError> {
    let secrets = state.secrets().await?;
    let raw = secrets
        .get(IGDB_CREDENTIALS)?
        .ok_or(AppError::MissingIgdbCredentials)?;
    let credentials: IgdbCredentials = serde_json::from_str(&raw)?;

    let cached: Option<IgdbToken> = secrets
        .get(IGDB_TOKEN)?
        .and_then(|raw| serde_json::from_str(&raw).ok());

    if let Some(token) = cached
        && token.is_valid(OffsetDateTime::now_utc())
    {
        return Ok((credentials, token));
    }

    let token = state.igdb.token(&credentials).await?;
    secrets.set(IGDB_TOKEN, &serde_json::to_string(&token)?)?;
    Ok((credentials, token))
}

#[derive(Serialize)]
pub struct ReviewItem {
    pub store_entry_id: String,
    pub store: &'static str,
    pub title: String,
    /// Lo que enseña la tienda de esta copia. Es la otra mitad de la
    /// comparación: sin ella el usuario decide entre candidatos de IGDB a
    /// ciegas, sin ver contra qué los está comparando.
    pub cover_url: Option<String>,
    pub store_url: Option<String>,
    pub candidates: Vec<ScoredCandidate>,
    /// Los dos mejores candidatos puntúan igual.
    ///
    /// Es con diferencia el motivo más común de acabar en esta cola, y no
    /// significa que el emparejamiento dude entre dos juegos distintos: IGDB
    /// tiene fichas duplicadas y las ediciones de un mismo juego se normalizan
    /// al mismo título. Se marca para poder agruparlas y resolverlas de una
    /// tacada en vez de una por una.
    pub tie: bool,
}

/// Cuándo se consideran empatados dos candidatos. El mismo margen que usa el
/// dominio para negarse a decidir, para que la cola agrupe exactamente lo que
/// el emparejamiento rechazó por ambiguo.
fn is_tie(candidates: &[ScoredCandidate]) -> bool {
    match (candidates.first(), candidates.get(1)) {
        (Some(best), Some(second)) => {
            best.score - second.score < domain::matching::AMBIGUITY_MARGIN
        }
        _ => false,
    }
}

/// La cola de revisión: lo que el emparejamiento automático no se atrevió a
/// decidir, con lo que encontró, para que el usuario elija sin buscar él.
#[tauri::command]
pub async fn review_queue(state: State<'_, AppState>) -> Result<Vec<ReviewItem>, AppError> {
    let entries = StoreEntryRepository(&state.db).unlinked().await?;
    let candidates = MatchCandidateRepository(&state.db);

    let mut queue = Vec::with_capacity(entries.len());
    for entry in entries {
        let found = candidates.for_entry(entry.id).await?;
        queue.push(ReviewItem {
            store_entry_id: entry.id.as_uuid().to_string(),
            store: entry.store.as_str(),
            title: entry.title.clone(),
            cover_url: entry.cover_url.clone(),
            store_url: entry.store_url.clone(),
            tie: is_tie(&found),
            candidates: found,
        });
    }
    Ok(queue)
}

/// Confirma varios emparejamientos de una vez.
///
/// No es el emparejamiento automático por la puerta de atrás. La interfaz trae
/// ya elegido el mejor candidato de lo que **no** empata, porque repetir con un
/// clic lo que la pantalla ya dice es trabajo inventado; pero eso se enseña en
/// una columna y no se escribe hasta que el usuario confirma. Lo que empata
/// sigue llegando sin elegir, que es exactamente lo que el umbral se negó a
/// decidir. Cada par queda como enlace `manual`, que ningún algoritmo volverá a
/// tocar. Lo único que ahorra es repetir el mismo gesto ciento cincuenta veces.
#[tauri::command]
pub async fn review_confirm_many(
    state: State<'_, AppState>,
    decisions: Vec<(String, i64)>,
) -> Result<usize, AppError> {
    let mut hechos = 0;
    for (store_entry_id, igdb_id) in decisions {
        confirm_one(&state, &store_entry_id, igdb_id).await?;
        hechos += 1;
    }
    Ok(hechos)
}

/// El usuario elige una ficha. Queda como enlace manual, y ningún
/// re-emparejamiento automático volverá a tocarlo.
#[tauri::command]
pub async fn review_confirm(
    state: State<'_, AppState>,
    store_entry_id: String,
    igdb_id: i64,
) -> Result<(), AppError> {
    confirm_one(&state, &store_entry_id, igdb_id).await
}

/// El cuerpo que comparten confirmar uno y confirmar muchos. Que sea el mismo
/// es lo que garantiza que el lote no tome ningún atajo respecto al de uno en
/// uno: crea la misma ficha y escribe el mismo enlace `manual`.
async fn confirm_one(state: &AppState, store_entry_id: &str, igdb_id: i64) -> Result<(), AppError> {
    let entry_id = parse_entry_id(store_entry_id)?;
    let entry = StoreEntryRepository(&state.db)
        .find(entry_id)
        .await?
        .ok_or_else(|| AppError::Message("esa entrada ya no existe".to_owned()))?;

    let games = GameRepository(&state.db);
    let game_id = match games.find_by_igdb(igdb_id).await? {
        Some(existing) => existing.id,
        None => {
            let (credentials, token) = igdb_session(state).await?;
            let meta = state.igdb.game(&credentials, &token, igdb_id).await?;
            let game = match meta {
                Some(meta) => domain::Game {
                    id: domain::GameId::new(),
                    canonical_title: meta.name.clone(),
                    sort_title: domain::matching::normalize(&meta.name),
                    igdb_id: Some(meta.igdb_id),
                    cover_url: meta.cover_url,
                    summary: meta.summary,
                    released_at: meta.released_at,
                    genres: meta.genres,
                },
                None => identity::local_game(&entry),
            };
            games.upsert(&game).await?;
            game.id
        }
    };

    link_manually(state, entry_id, game_id).await
}

/// «Este juego no está en IGDB»: se le crea una ficha con el título de la
/// tienda para que deje de aparecer en la cola y pueda tener estado como
/// cualquier otro.
#[tauri::command]
pub async fn review_without_metadata(
    state: State<'_, AppState>,
    store_entry_id: String,
) -> Result<(), AppError> {
    let entry_id = parse_entry_id(&store_entry_id)?;
    let entry = StoreEntryRepository(&state.db)
        .find(entry_id)
        .await?
        .ok_or_else(|| AppError::Message("esa entrada ya no existe".to_owned()))?;

    let game = identity::local_game(&entry);
    GameRepository(&state.db).upsert(&game).await?;
    link_manually(&state, entry_id, game.id).await
}

async fn link_manually(
    state: &AppState,
    entry_id: StoreEntryId,
    game_id: domain::GameId,
) -> Result<(), AppError> {
    GameLinkRepository(&state.db)
        .set_manual(&GameLink {
            game_id,
            store_entry_id: entry_id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await?;
    MatchCandidateRepository(&state.db).clear(entry_id).await?;
    Ok(())
}

fn parse_entry_id(raw: &str) -> Result<StoreEntryId, AppError> {
    Uuid::parse_str(raw)
        .map(StoreEntryId::from_uuid)
        .map_err(|_| AppError::Message("identificador de entrada inválido".to_owned()))
}
