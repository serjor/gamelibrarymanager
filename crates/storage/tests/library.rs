//! La consulta de biblioteca: que junte bien lo que cuelga de una ficha
//! —tiendas, horas, estado, géneros— aunque venga de varias tiendas.
//!
//! Que sea **una sola** consulta se prueba aparte, en `una_sola_consulta.rs`,
//! porque contar sentencias necesita un binario para él solo.

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, PlayStatus, StoreAccount, StoreAccountId,
    StoreEntry, StoreEntryId, StoreId, UserState,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, StoreAccountRepository,
    StoreEntryRepository, UserStateRepository,
};
use time::OffsetDateTime;

async fn cuenta(db: &Database, store: StoreId) -> StoreAccountId {
    StoreAccountRepository(db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store,
            account_ref: format!("cuenta-{}", store.as_str()),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("cuenta")
}

fn juego(title: &str, genres: &[&str]) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id: None,
        cover_url: Some("https://images.igdb.com/x.jpg".to_owned()),
        summary: None,
        released_at: OffsetDateTime::from_unix_timestamp(1_571_270_400).ok(),
        genres: genres.iter().map(|g| (*g).to_owned()).collect(),
    }
}

fn entrada(
    account_id: StoreAccountId,
    store: StoreId,
    app_id: &str,
    kind: EntryKind,
    playtime: i64,
) -> StoreEntry {
    StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind,
        title: "da igual".to_owned(),
        playtime_minutes: Some(playtime),
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    }
}

/// Lo que de verdad guarda el conector de Steam, que es de donde sale la última
/// partida: `steam/parse.rs` mete `rtime_last_played` en el JSON crudo.
fn como_la_guarda_steam(mut entry: StoreEntry, ultima: i64) -> StoreEntry {
    entry.cover_url = Some("https://cdn.cloudflare.steamstatic.com/header.jpg".to_owned());
    entry.store_url = Some("https://store.steampowered.com/app/632470".to_owned());
    entry.raw = serde_json::json!({ "appid": 632_470, "rtime_last_played": ultima });
    entry
}

#[tokio::test]
async fn una_ficha_con_dos_tiendas_suma_horas_y_lista_ambas() {
    let db = Database::in_memory().await.expect("base");
    let steam = cuenta(&db, StoreId::Steam).await;
    let gog = cuenta(&db, StoreId::Gog).await;

    let ficha = juego("Disco Elysium", &["RPG", "Aventura"]);
    GameRepository(&db).upsert(&ficha).await.expect("ficha");

    let en_steam = como_la_guarda_steam(
        entrada(steam, StoreId::Steam, "632470", EntryKind::Owned, 1200),
        1_700_000_000,
    );
    let mut en_gog = entrada(gog, StoreId::Gog, "151239", EntryKind::Owned, 300);
    en_gog.cover_url = Some("https://images.gog-statics.com/logo.jpg".to_owned());
    en_gog.store_url = Some("https://www.gog.com/game/disco_elysium".to_owned());
    let deseado = entrada(gog, StoreId::Gog, "999", EntryKind::Wishlist, 0);
    StoreEntryRepository(&db)
        .upsert_many(&[en_steam.clone(), en_gog.clone(), deseado.clone()])
        .await
        .expect("entradas");

    GameLinkRepository(&db)
        .rebuild_auto(&[
            GameLink {
                game_id: ficha.id,
                store_entry_id: en_steam.id,
                confidence: 1.0,
                method: LinkMethod::Auto,
            },
            GameLink {
                game_id: ficha.id,
                store_entry_id: en_gog.id,
                confidence: 1.0,
                method: LinkMethod::Auto,
            },
            GameLink {
                game_id: ficha.id,
                store_entry_id: deseado.id,
                confidence: 1.0,
                method: LinkMethod::Auto,
            },
        ])
        .await
        .expect("enlaces");

    UserStateRepository(&db)
        .save(&UserState {
            game_id: ficha.id,
            status: Some(PlayStatus::Playing),
            rating: Some(10),
            notes: Some("una maravilla".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("estado");

    let rows = LibraryRepository(&db).all().await.expect("biblioteca");
    assert_eq!(rows.len(), 1, "dos copias son un solo juego");

    let row = &rows[0];
    assert_eq!(row.owned_stores, vec!["gog".to_owned(), "steam".to_owned()]);
    assert_eq!(row.wishlist_stores, vec!["gog".to_owned()]);
    assert_eq!(
        row.playtime_minutes, 1500,
        "las horas de las dos tiendas se suman"
    );
    assert_eq!(row.status, Some(PlayStatus::Playing));
    assert_eq!(row.rating, Some(10));
    assert_eq!(row.genres, vec!["RPG".to_owned(), "Aventura".to_owned()]);
    assert_eq!(row.release_year, Some(2019));

    // Con copia en las dos tiendas, la imagen y el enlace salen los dos de
    // Steam: su cabecera está pensada para verse apaisada y GOG solo da el
    // logo. Que salgan de la misma copia es lo que evita enseñar la imagen de
    // una tienda con el enlace de la otra.
    assert_eq!(
        row.store_cover_url.as_deref(),
        Some("https://cdn.cloudflare.steamstatic.com/header.jpg")
    );
    assert_eq!(
        row.store_url.as_deref(),
        Some("https://store.steampowered.com/app/632470")
    );
    assert_eq!(
        row.last_played_at,
        Some(1_700_000_000),
        "la última partida sale del JSON crudo de Steam"
    );
}

#[tokio::test]
async fn sin_copia_en_steam_no_hay_ultima_partida() {
    // GOG no publica ni horas ni fecha de la última partida, así que la columna
    // se queda vacía por mucho que el juego se haya jugado. Es una carencia de
    // la tienda, no un fallo: la interfaz tiene que poder distinguirlo de un
    // «nunca jugado» y por eso es `None` y no un cero.
    let db = Database::in_memory().await.expect("base");
    let gog = cuenta(&db, StoreId::Gog).await;

    let ficha = juego("Cultist Simulator", &["Simulación"]);
    GameRepository(&db).upsert(&ficha).await.expect("ficha");

    let en_gog = entrada(gog, StoreId::Gog, "1207660103", EntryKind::Owned, 90);
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&en_gog))
        .await
        .expect("entradas");
    GameLinkRepository(&db)
        .rebuild_auto(&[GameLink {
            game_id: ficha.id,
            store_entry_id: en_gog.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        }])
        .await
        .expect("enlaces");

    let rows = LibraryRepository(&db).all().await.expect("biblioteca");
    assert_eq!(rows[0].last_played_at, None);
    assert_eq!(rows[0].playtime_minutes, 90, "las horas sí las da GOG");
}

#[tokio::test]
async fn steam_sin_estrenar_no_cuenta_como_jugado_en_1970() {
    // Steam manda `rtime_last_played: 0` para lo que nunca se ha abierto. Sin
    // convertirlo a NULL, ordenar por última partida pondría los juegos sin
    // estrenar como los más antiguos de la biblioteca en vez de aparte.
    let db = Database::in_memory().await.expect("base");
    let steam = cuenta(&db, StoreId::Steam).await;

    let ficha = juego("Prey", &["Acción"]);
    GameRepository(&db).upsert(&ficha).await.expect("ficha");

    let sin_estrenar = como_la_guarda_steam(
        entrada(steam, StoreId::Steam, "480490", EntryKind::Owned, 0),
        0,
    );
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&sin_estrenar))
        .await
        .expect("entradas");
    GameLinkRepository(&db)
        .rebuild_auto(&[GameLink {
            game_id: ficha.id,
            store_entry_id: sin_estrenar.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        }])
        .await
        .expect("enlaces");

    let rows = LibraryRepository(&db).all().await.expect("biblioteca");
    assert_eq!(rows[0].last_played_at, None);
}
