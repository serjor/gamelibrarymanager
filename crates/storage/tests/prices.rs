//! Los precios son la quinta capa, y la única cuyas filas se borran de verdad.
//! Lo que estos tests vigilan es que ese borrado no llegue nunca ni a la copia
//! de la tienda ni a lo que escribió el usuario.

use domain::{
    Deal, EntryKind, Game, GameId, GameLink, GamePrices, LinkMethod, Money, PlayStatus,
    StoreAccount, StoreAccountId, StoreEntry, StoreEntryId, StoreId, UserState,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, PriceRepository, StoreAccountRepository,
    StoreEntryRepository, UserStateRepository,
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

/// Una ficha con una copia colgando, del tipo que se pida.
async fn juego(db: &Database, store: StoreId, app_id: &str, kind: EntryKind) -> GameId {
    let account_id = account(db, store).await;
    let entry = StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind,
        title: "Disco Elysium".to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    };
    StoreEntryRepository(db)
        .upsert_many(std::slice::from_ref(&entry))
        .await
        .expect("alta de copia");

    let game = Game {
        id: GameId::new(),
        canonical_title: "Disco Elysium".to_owned(),
        sort_title: "disco elysium".to_owned(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    GameRepository(db)
        .upsert(&game)
        .await
        .expect("alta de ficha");

    // `rebuild_auto` reescribe **todos** los enlaces automáticos, así que hay
    // que partir de los que ya hay: pasar solo el nuevo dejaría sin ficha a los
    // juegos que creó la llamada anterior.
    let mut links = GameLinkRepository(db).all().await.expect("enlaces");
    links.push(GameLink {
        game_id: game.id,
        store_entry_id: entry.id,
        confidence: 1.0,
        method: LinkMethod::Auto,
    });
    GameLinkRepository(db)
        .rebuild_auto(&links)
        .await
        .expect("enlace");

    game.id
}

fn euros(cents: i64) -> Money {
    Money {
        cents,
        currency: "EUR".to_owned(),
    }
}

fn oferta(shop: &str, cents: i64, cut: i64) -> Deal {
    Deal {
        shop: shop.to_owned(),
        price: euros(cents),
        regular: euros(3999),
        cut,
    }
}

fn precios(deals: Vec<Deal>) -> GamePrices {
    GamePrices {
        provider_id: "018d937f-0e3f-72d4-a1a2-6d0e0b0f9d2c".to_owned(),
        low_all_time: Some(euros(899)),
        low_year: Some(euros(1349)),
        deals,
    }
}

#[tokio::test]
async fn solo_los_deseados_entran_en_la_consulta_de_precios() {
    let db = Database::in_memory().await.expect("base");
    let deseado = juego(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    juego(&db, StoreId::Gog, "1207658930", EntryKind::Owned).await;

    let targets = PriceRepository(&db).targets().await.expect("objetivos");

    assert_eq!(
        targets.len(),
        1,
        "un juego en propiedad no tiene precio que mirar"
    );
    assert_eq!(targets[0].game_id, deseado);
    // El appid es lo que convierte la búsqueda en exacta: sin él, ITAD decide
    // por el título y puede equivocarse.
    assert_eq!(targets[0].steam_app_id.as_deref(), Some("632470"));
    assert_eq!(targets[0].itad_id, None);
}

#[tokio::test]
async fn el_identificador_de_itad_se_recuerda_y_sobrevive_a_enriquecer_la_ficha() {
    let db = Database::in_memory().await.expect("base");
    let game_id = juego(&db, StoreId::Gog, "1207658930", EntryKind::Wishlist).await;

    GameRepository(&db)
        .set_itad(game_id, "018d937f", "disco-elysium")
        .await
        .expect("anotar itad");

    // Emparejar con IGDB reescribe la ficha entera. El identificador de precios
    // no puede irse por delante: la siguiente consulta volvería a gastar una
    // búsqueda por cada deseado.
    let mut game = GameRepository(&db)
        .find(game_id)
        .await
        .expect("ficha")
        .expect("existe");
    game.igdb_id = Some(115653);
    game.canonical_title = "Disco Elysium".to_owned();
    GameRepository(&db).upsert(&game).await.expect("enriquecer");

    let targets = PriceRepository(&db).targets().await.expect("objetivos");
    assert_eq!(targets[0].itad_id.as_deref(), Some("018d937f"));
}

#[tokio::test]
async fn el_precio_que_se_enseña_es_el_mas_barato_con_su_tienda() {
    let db = Database::in_memory().await.expect("base");
    let game_id = juego(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    let prices = PriceRepository(&db);

    prices
        .save(
            game_id,
            &precios(vec![
                oferta("Steam", 1799, 55),
                oferta("GOG", 1599, 60),
                oferta("Fanatical", 2099, 47),
            ]),
        )
        .await
        .expect("guardar precios");

    let rows = prices.all().await.expect("consultar precios");

    assert_eq!(rows.len(), 1, "una fila por juego, no una por tienda");
    assert_eq!(rows[0].shop, "GOG");
    assert_eq!(rows[0].amount, 1599);
    assert_eq!(rows[0].cut, 60);
    assert_eq!(rows[0].shops, 3);
    assert_eq!(rows[0].low_all_time, Some(899));
    assert_eq!(rows[0].low_year, Some(1349));
}

/// Dos tiendas al mismo céntimo. Sin desempate, la que se enseña cambiaría de
/// una apertura de la aplicación a la siguiente sin que nada hubiera cambiado.
#[tokio::test]
async fn un_empate_al_centimo_se_resuelve_siempre_igual() {
    let db = Database::in_memory().await.expect("base");
    let game_id = juego(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    let prices = PriceRepository(&db);

    prices
        .save(
            game_id,
            &precios(vec![oferta("Steam", 1599, 60), oferta("GOG", 1599, 60)]),
        )
        .await
        .expect("guardar precios");

    for _ in 0..5 {
        let rows = prices.all().await.expect("consultar precios");
        assert_eq!(rows[0].shop, "GOG");
    }
}

/// Refrescar sustituye, no acumula: la oferta que terminó tiene que desaparecer.
/// Una oferta caducada que se queda en pantalla sigue pareciendo una oferta.
#[tokio::test]
async fn refrescar_sustituye_las_ofertas_y_no_las_acumula() {
    let db = Database::in_memory().await.expect("base");
    let game_id = juego(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    let prices = PriceRepository(&db);

    prices
        .save(
            game_id,
            &precios(vec![oferta("Steam", 1799, 55), oferta("GOG", 1599, 60)]),
        )
        .await
        .expect("primera pasada");
    prices
        .save(game_id, &precios(vec![oferta("Steam", 3999, 0)]))
        .await
        .expect("segunda pasada");

    let rows = prices.all().await.expect("consultar precios");
    assert_eq!(
        rows[0].shops, 1,
        "la rebaja de GOG terminó y no puede seguir ahí"
    );
    assert_eq!(rows[0].shop, "Steam");
    assert_eq!(rows[0].amount, 3999);
}

#[tokio::test]
async fn un_juego_que_sale_de_la_lista_deja_de_tener_precio() {
    let db = Database::in_memory().await.expect("base");
    let comprado = juego(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    let sigue = juego(&db, StoreId::Gog, "1207658930", EntryKind::Wishlist).await;
    let prices = PriceRepository(&db);

    for game_id in [comprado, sigue] {
        prices
            .save(game_id, &precios(vec![oferta("Steam", 1799, 55)]))
            .await
            .expect("guardar precios");
    }

    prices.forget_missing(&[sigue]).await.expect("olvidar");

    let rows = prices.all().await.expect("consultar precios");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].game_id, sigue);
}

/// La garantía de la fase 2, aplicada a la capa nueva: los precios entran y
/// salen sin rozar lo que escribió el usuario ni lo que dijo la tienda.
#[tokio::test]
async fn los_precios_no_tocan_ni_la_copia_ni_el_estado_del_usuario() {
    let db = Database::in_memory().await.expect("base");
    let game_id = juego(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;

    UserStateRepository(&db)
        .save(&UserState {
            game_id,
            status: Some(PlayStatus::Backlog),
            rating: Some(9),
            notes: Some("esperando rebaja".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("estado");

    let prices = PriceRepository(&db);
    prices
        .save(game_id, &precios(vec![oferta("Steam", 1799, 55)]))
        .await
        .expect("guardar precios");
    prices.forget_missing(&[]).await.expect("olvidarlo todo");

    let estado = UserStateRepository(&db)
        .find(game_id)
        .await
        .expect("consultar estado")
        .expect("el estado sigue ahí");
    assert_eq!(estado.notes.as_deref(), Some("esperando rebaja"));
    assert_eq!(estado.rating, Some(9));
    assert_eq!(
        StoreEntryRepository(&db)
            .active(EntryKind::Wishlist)
            .await
            .expect("copias")
            .len(),
        1
    );
}
