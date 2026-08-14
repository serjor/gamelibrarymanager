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

#[tokio::test]
async fn una_ficha_con_dos_tiendas_suma_horas_y_lista_ambas() {
    let db = Database::in_memory().await.expect("base");
    let steam = cuenta(&db, StoreId::Steam).await;
    let gog = cuenta(&db, StoreId::Gog).await;

    let ficha = juego("Disco Elysium", &["RPG", "Aventura"]);
    GameRepository(&db).upsert(&ficha).await.expect("ficha");

    let en_steam = entrada(steam, StoreId::Steam, "632470", EntryKind::Owned, 1200);
    let en_gog = entrada(gog, StoreId::Gog, "151239", EntryKind::Owned, 300);
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
}
