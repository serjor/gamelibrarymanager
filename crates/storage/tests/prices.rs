//! The prices are the fifth layer, and the only layer whose rows are really
//! deleted. What these tests watch is that the delete never reaches the copy of
//! the store or what the user wrote.

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
            account_ref: format!("account-{}", store.as_str()),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("add the account")
}

/// A record with one copy attached, of the kind requested.
async fn game(db: &Database, store: StoreId, app_id: &str, kind: EntryKind) -> GameId {
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
        .expect("add the copy");

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
        .expect("add the record");

    // `rebuild_auto` writes **all** of the automatic links again, thus you must
    // start from the links that are already there: to give only the new one
    // would leave the games of the earlier call with no record.
    let mut links = GameLinkRepository(db).all().await.expect("links");
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

fn prices(deals: Vec<Deal>) -> GamePrices {
    GamePrices {
        provider_id: "018d937f-0e3f-72d4-a1a2-6d0e0b0f9d2c".to_owned(),
        low_all_time: Some(euros(899)),
        low_year: Some(euros(1349)),
        deals,
    }
}

#[tokio::test]
async fn only_the_wished_for_games_go_into_the_price_query() {
    let db = Database::in_memory().await.expect("database");
    let wished = game(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    game(&db, StoreId::Gog, "1207658930", EntryKind::Owned).await;

    let targets = PriceRepository(&db).targets().await.expect("objetivos");

    assert_eq!(targets.len(), 1, "an owned game has no price to look at");
    assert_eq!(targets[0].game_id, wished);
    // The appid is what makes the search exact: without it, ITAD decides by the
    // title and can be incorrect.
    assert_eq!(targets[0].steam_app_id.as_deref(), Some("632470"));
    assert_eq!(targets[0].itad_id, None);
}

#[tokio::test]
async fn the_itad_identifier_is_kept_and_survives_the_metadata_of_the_record() {
    let db = Database::in_memory().await.expect("database");
    let game_id = game(&db, StoreId::Gog, "1207658930", EntryKind::Wishlist).await;

    GameRepository(&db)
        .set_itad(game_id, "018d937f", "disco-elysium")
        .await
        .expect("anotar itad");

    // A match with IGDB writes all of the record again. The price identifier
    // cannot go with it: the next query would spend one search for each
    // wished-for game again.
    let mut game = GameRepository(&db)
        .find(game_id)
        .await
        .expect("record")
        .expect("existe");
    game.igdb_id = Some(115653);
    game.canonical_title = "Disco Elysium".to_owned();
    GameRepository(&db).upsert(&game).await.expect("enriquecer");

    let targets = PriceRepository(&db).targets().await.expect("objetivos");
    assert_eq!(targets[0].itad_id.as_deref(), Some("018d937f"));
}

#[tokio::test]
async fn the_price_shown_is_the_least_expensive_with_its_store() {
    let db = Database::in_memory().await.expect("database");
    let game_id = game(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    let repo = PriceRepository(&db);

    repo.save(
        game_id,
        &prices(vec![
            oferta("Steam", 1799, 55),
            oferta("GOG", 1599, 60),
            oferta("Fanatical", 2099, 47),
        ]),
    )
    .await
    .expect("guardar prices");

    let rows = repo.all().await.expect("consultar prices");

    assert_eq!(
        rows.len(),
        1,
        "one row for each game, not one for each store"
    );
    assert_eq!(rows[0].shop, "GOG");
    assert_eq!(rows[0].amount, 1599);
    assert_eq!(rows[0].cut, 60);
    assert_eq!(rows[0].shops, 3);
    assert_eq!(rows[0].low_all_time, Some(899));
    assert_eq!(rows[0].low_year, Some(1349));
}

/// Two stores at the same cent. With no tie break, the store shown would change
/// from one start of the application to the next with no change in the data.
#[tokio::test]
async fn a_tie_at_the_cent_always_resolves_in_the_same_way() {
    let db = Database::in_memory().await.expect("database");
    let game_id = game(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    let repo = PriceRepository(&db);

    repo.save(
        game_id,
        &prices(vec![oferta("Steam", 1599, 60), oferta("GOG", 1599, 60)]),
    )
    .await
    .expect("guardar prices");

    for _ in 0..5 {
        let rows = repo.all().await.expect("consultar prices");
        assert_eq!(rows[0].shop, "GOG");
    }
}

/// A refresh replaces, it does not accumulate: the offer that ended must go.
/// An expired offer that stays on the screen still looks like an offer.
#[tokio::test]
async fn a_refresh_replaces_the_offers_and_does_not_accumulate_them() {
    let db = Database::in_memory().await.expect("database");
    let game_id = game(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    let repo = PriceRepository(&db);

    repo.save(
        game_id,
        &prices(vec![oferta("Steam", 1799, 55), oferta("GOG", 1599, 60)]),
    )
    .await
    .expect("first pass");
    repo.save(game_id, &prices(vec![oferta("Steam", 3999, 0)]))
        .await
        .expect("second pass");

    let rows = repo.all().await.expect("consultar prices");
    assert_eq!(
        rows[0].shops, 1,
        "the GOG discount ended and cannot stay there"
    );
    assert_eq!(rows[0].shop, "Steam");
    assert_eq!(rows[0].amount, 3999);
}

#[tokio::test]
async fn a_game_that_leaves_the_list_stops_having_a_price() {
    let db = Database::in_memory().await.expect("database");
    let comprado = game(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;
    let stays = game(&db, StoreId::Gog, "1207658930", EntryKind::Wishlist).await;
    let repo = PriceRepository(&db);

    for game_id in [comprado, stays] {
        repo.save(game_id, &prices(vec![oferta("Steam", 1799, 55)]))
            .await
            .expect("guardar prices");
    }

    repo.forget_missing(&[stays]).await.expect("olvidar");

    let rows = repo.all().await.expect("consultar prices");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].game_id, stays);
}

/// The guarantee of phase 2, applied to the new layer: the prices come in and go
/// out and do not touch what the user wrote or what the store said.
#[tokio::test]
async fn the_prices_touch_neither_the_copy_nor_the_user_status() {
    let db = Database::in_memory().await.expect("database");
    let game_id = game(&db, StoreId::Steam, "632470", EntryKind::Wishlist).await;

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
        .expect("status");

    let repo = PriceRepository(&db);
    repo.save(game_id, &prices(vec![oferta("Steam", 1799, 55)]))
        .await
        .expect("guardar prices");
    repo.forget_missing(&[]).await.expect("olvidarlo todo");

    let state = UserStateRepository(&db)
        .find(game_id)
        .await
        .expect("consultar state")
        .expect("the status stays there");
    assert_eq!(state.notes.as_deref(), Some("esperando rebaja"));
    assert_eq!(state.rating, Some(9));
    assert_eq!(
        StoreEntryRepository(&db)
            .active(EntryKind::Wishlist)
            .await
            .expect("copias")
            .len(),
        1
    );
}
