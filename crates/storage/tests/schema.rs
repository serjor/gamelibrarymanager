//! Los dos desastres que la separación en cuatro capas debe impedir:
//! que re-emparejar borre lo que escribió el usuario, y que el emparejamiento
//! automático pise una corrección manual.

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, PlayStatus, StoreAccount, StoreAccountId,
    StoreEntry, StoreEntryId, StoreId, UserState,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, StoreAccountRepository, StoreEntryRepository,
    UserStateRepository,
};
use time::OffsetDateTime;

async fn account(db: &Database, store: StoreId) -> StoreAccountId {
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
        .expect("alta de cuenta")
}

fn entry(account_id: StoreAccountId, store: StoreId, app_id: &str) -> StoreEntry {
    StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind: EntryKind::Owned,
        title: "Disco Elysium".to_owned(),
        playtime_minutes: Some(1200),
        acquired_at: None,
        raw: serde_json::json!({ "appid": app_id }),
    }
}

#[tokio::test]
async fn las_migraciones_aplican_y_revierten() {
    let db = Database::in_memory().await.expect("abrir base");

    // Aplicadas por `open`: las tablas responden.
    assert_eq!(GameRepository(&db).all().await.expect("consultar").len(), 0);

    db.undo_all().await.expect("revertir migraciones");

    // Revertidas: la tabla ya no existe y la consulta falla.
    assert!(
        GameRepository(&db).all().await.is_err(),
        "tras revertir no debe quedar esquema"
    );
}

#[tokio::test]
async fn reemparejar_no_toca_el_estado_del_usuario() {
    let db = Database::in_memory().await.expect("abrir base");
    let steam = account(&db, StoreId::Steam).await;
    let gog = account(&db, StoreId::Gog).await;

    // El mismo juego, comprado en dos tiendas.
    let en_steam = entry(steam, StoreId::Steam, "632470");
    let en_gog = entry(gog, StoreId::Gog, "1512395083");
    StoreEntryRepository(&db)
        .upsert_many(&[en_steam.clone(), en_gog.clone()])
        .await
        .expect("volcar entradas");

    // Una sola ficha para las dos copias.
    let game_id = GameId::new();
    GameRepository(&db)
        .upsert(&Game {
            id: game_id,
            canonical_title: "Disco Elysium".to_owned(),
            sort_title: "disco elysium".to_owned(),
            igdb_id: Some(115_653),
            cover_url: None,
            summary: None,
            released_at: None,
            genres: Vec::new(),
        })
        .await
        .expect("alta de ficha");

    let links = vec![
        GameLink {
            game_id,
            store_entry_id: en_steam.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        },
        GameLink {
            game_id,
            store_entry_id: en_gog.id,
            confidence: 0.9,
            method: LinkMethod::Auto,
        },
    ];
    let repo = GameLinkRepository(&db);
    repo.rebuild_auto(&links).await.expect("emparejar");
    assert_eq!(repo.for_game(game_id).await.expect("leer enlaces").len(), 2);
    assert_eq!(repo.unlinked_entry_count().await.expect("contar"), 0);

    // El usuario anota su estado.
    UserStateRepository(&db)
        .save(&UserState {
            game_id,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("por el capítulo 3".to_owned()),
            started_at: Some(OffsetDateTime::now_utc()),
            finished_at: None,
        })
        .await
        .expect("guardar estado");

    // Se rehace el emparejamiento entero, dos veces.
    repo.rebuild_auto(&links).await.expect("re-emparejar");
    repo.rebuild_auto(&links)
        .await
        .expect("re-emparejar de nuevo");

    let state = UserStateRepository(&db)
        .find(game_id)
        .await
        .expect("leer estado")
        .expect("el estado del usuario debe sobrevivir al re-emparejamiento");
    assert_eq!(state.status, Some(PlayStatus::Playing));
    assert_eq!(state.rating, Some(9));
    assert_eq!(state.notes.as_deref(), Some("por el capítulo 3"));

    // Y el dato original de la tienda sigue intacto.
    let almacenada = StoreEntryRepository(&db)
        .find(en_steam.id)
        .await
        .expect("leer entrada")
        .expect("la entrada de tienda no se toca al re-emparejar");
    assert_eq!(almacenada.raw, en_steam.raw);
}

#[tokio::test]
async fn el_emparejamiento_automatico_no_pisa_una_correccion_manual() {
    let db = Database::in_memory().await.expect("abrir base");
    let steam = account(&db, StoreId::Steam).await;
    let en_steam = entry(steam, StoreId::Steam, "632470");
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&en_steam))
        .await
        .expect("volcar entrada");

    let equivocado = GameId::new();
    let correcto = GameId::new();
    for (id, titulo) in [
        (equivocado, "Disco Elysium"),
        (correcto, "Disco Elysium: The Final Cut"),
    ] {
        GameRepository(&db)
            .upsert(&Game {
                id,
                canonical_title: titulo.to_owned(),
                sort_title: titulo.to_lowercase(),
                igdb_id: None,
                cover_url: None,
                summary: None,
                released_at: None,
                genres: Vec::new(),
            })
            .await
            .expect("alta de ficha");
    }

    let repo = GameLinkRepository(&db);
    // El usuario corrige a mano.
    repo.set_manual(&GameLink {
        game_id: correcto,
        store_entry_id: en_steam.id,
        confidence: 1.0,
        method: LinkMethod::Manual,
    })
    .await
    .expect("corrección manual");

    // El automático vuelve a proponer lo suyo, y no debe ganar.
    repo.rebuild_auto(&[GameLink {
        game_id: equivocado,
        store_entry_id: en_steam.id,
        confidence: 0.95,
        method: LinkMethod::Auto,
    }])
    .await
    .expect("re-emparejar");

    let enlaces = repo.all().await.expect("leer enlaces");
    assert_eq!(enlaces.len(), 1, "una entrada solo puede tener un enlace");
    assert_eq!(enlaces[0].game_id, correcto, "la corrección manual manda");
    assert_eq!(enlaces[0].method, LinkMethod::Manual);
}

#[tokio::test]
async fn sincronizar_dos_veces_no_duplica() {
    let db = Database::in_memory().await.expect("abrir base");
    let steam = account(&db, StoreId::Steam).await;
    let primera = entry(steam, StoreId::Steam, "632470");

    let repo = StoreEntryRepository(&db);
    repo.upsert_many(std::slice::from_ref(&primera))
        .await
        .expect("primera");

    // Segunda sincronización: mismo juego, id nuevo y más horas jugadas.
    let mut segunda = entry(steam, StoreId::Steam, "632470");
    segunda.playtime_minutes = Some(1500);
    repo.upsert_many(&[segunda]).await.expect("segunda");

    let activas = repo.active(EntryKind::Owned).await.expect("listar");
    assert_eq!(activas.len(), 1, "no puede haber dos filas del mismo juego");
    assert_eq!(activas[0].id, primera.id, "se conserva la fila original");
    assert_eq!(activas[0].playtime_minutes, Some(1500));
}

#[tokio::test]
async fn lo_que_desaparece_de_la_tienda_se_marca_pero_no_se_borra() {
    let db = Database::in_memory().await.expect("abrir base");
    let steam = account(&db, StoreId::Steam).await;
    let retirado = entry(steam, StoreId::Steam, "632470");
    let sigue = entry(steam, StoreId::Steam, "292030");

    let repo = StoreEntryRepository(&db);
    repo.upsert_many(&[retirado.clone(), sigue.clone()])
        .await
        .expect("volcar");

    let bajas = repo
        .soft_delete_missing(steam, EntryKind::Owned, &["292030".to_owned()])
        .await
        .expect("baja lógica");

    assert_eq!(bajas, 1);
    assert_eq!(
        repo.active(EntryKind::Owned).await.expect("listar").len(),
        1
    );
    assert!(
        repo.find(retirado.id).await.expect("buscar").is_some(),
        "la fila sigue existiendo, solo está marcada"
    );
}
