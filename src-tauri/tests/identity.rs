//! The complete cycle of phase 4 against a pretend IGDB and a real database:
//! an exact appid, an unsure title, and the guarantee that a new match does not
//! touch what the user decided by hand.

use domain::{
    EntryKind, GameLink, LinkMethod, StoreAccount, StoreAccountId, StoreEntry, StoreEntryId,
    StoreId,
};
use gamelibrarymanager_lib::testing::{Silent, resolve};
use metadata::IgdbClient;
use metadata::igdb::{IgdbCredentials, IgdbToken};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, MatchCandidateRepository, StoreAccountRepository,
    StoreEntryRepository,
};
use time::OffsetDateTime;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EXTERNAL: &str = r#"[{"id":1,"uid":"632470","game":115653}]"#;
const GAME_115653: &str = r#"[{"id":115653,"name":"Disco Elysium","first_release_date":1571270400,
                              "cover":{"id":1,"image_id":"co1x2y"}}]"#;
const SEARCH_AMBIGUO: &str = r#"[{"id":250,"name":"Doom","first_release_date":757382400},
                                 {"id":7351,"name":"Doom","first_release_date":1463011200}]"#;

fn credentials() -> IgdbCredentials {
    IgdbCredentials {
        client_id: "id".to_owned(),
        client_secret: "secreto".to_owned(),
    }
}

fn token() -> IgdbToken {
    IgdbToken {
        access_token: "token".to_owned(),
        expires_at: OffsetDateTime::now_utc().unix_timestamp() + 3600,
    }
}

async fn igdb_server(search_body: &'static str) -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains("\"632470\""))
        .respond_with(ResponseTemplate::new(200).set_body_raw(EXTERNAL, "application/json"))
        .mount(&server)
        .await;
    // Every other appid is unknown to IGDB.
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    // The concrete record and the search share an endpoint: the body of the
    // query tells them apart.
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("where id = 115653"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GAME_115653, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("search"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(search_body, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    server
}

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

fn entry(account_id: StoreAccountId, store: StoreId, app_id: &str, title: &str) -> StoreEntry {
    StoreEntry {
        id: StoreEntryId::new(),
        account_id,
        store,
        store_app_id: app_id.to_owned(),
        kind: EntryKind::Owned,
        title: title.to_owned(),
        playtime_minutes: None,
        acquired_at: None,
        cover_url: None,
        store_url: None,
        raw: serde_json::json!({}),
    }
}

#[tokio::test]
async fn the_steam_appid_links_with_no_question_and_the_unsure_title_goes_to_the_queue() {
    let db = Database::in_memory().await.expect("database");
    let steam = account(&db, StoreId::Steam).await;
    let gog = account(&db, StoreId::Gog).await;

    let exacto = entry(steam, StoreId::Steam, "632470", "Disco Elysium");
    let unsure = entry(gog, StoreId::Gog, "1234", "Doom");
    StoreEntryRepository(&db)
        .upsert_many(&[exacto.clone(), unsure.clone()])
        .await
        .expect("write the entries");

    let server = igdb_server(SEARCH_AMBIGUO).await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    let report = resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("match");

    assert_eq!(report.linked, 1, "the exact appid links alone");
    assert_eq!(
        report.review, 1,
        "two Doom with the same name are not decided alone"
    );

    let links = GameLinkRepository(&db).all().await.expect("links");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].store_entry_id, exacto.id);
    assert_eq!(
        links[0].confidence, 1.0,
        "an external identifier accepts no degrees of confidence"
    );

    // The record was made with the IGDB metadata, not with the store title.
    let games = GameRepository(&db).all().await.expect("records");
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].igdb_id, Some(115653));
    assert!(games[0].cover_url.is_some(), "la portada viene de IGDB");

    // And the unsure entry stayed in the queue with its candidates, for the user.
    let candidates = MatchCandidateRepository(&db)
        .for_entry(unsure.id)
        .await
        .expect("candidates");
    assert_eq!(candidates.len(), 2);
}

#[tokio::test]
async fn a_new_match_does_not_change_a_manual_link() {
    let db = Database::in_memory().await.expect("database");
    let gog = account(&db, StoreId::Gog).await;
    let unsure = entry(gog, StoreId::Gog, "1234", "Doom");
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&unsure))
        .await
        .expect("write the entry");

    let server = igdb_server(SEARCH_AMBIGUO).await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    // Primera pasada: a la cola.
    resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("match");
    assert!(
        GameLinkRepository(&db)
            .all()
            .await
            .expect("links")
            .is_empty()
    );

    // The user decides: it is the Doom of 2016.
    let chosen = domain::Game {
        id: domain::GameId::new(),
        canonical_title: "Doom".to_owned(),
        sort_title: "doom".to_owned(),
        igdb_id: Some(7351),
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    GameRepository(&db).upsert(&chosen).await.expect("record");
    GameLinkRepository(&db)
        .set_manual(&GameLink {
            game_id: chosen.id,
            store_entry_id: unsure.id,
            confidence: 1.0,
            method: LinkMethod::Manual,
        })
        .await
        .expect("enlace manual");

    // It matches again, two more times.
    for _ in 0..2 {
        resolve(&db, &igdb, &credentials(), &token(), &Silent)
            .await
            .expect("match again");
    }

    let links = GameLinkRepository(&db).all().await.expect("links");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].method, LinkMethod::Manual, "stays siendo manual");
    assert_eq!(
        links[0].game_id, chosen.id,
        "and it still points at what the user selected"
    );
}

/// A stop from IGDB in the middle cannot lose all of the pass.
///
/// That is what occurred: the links were written only at the end, thus a 429 at
/// game three hundred — five minutes of request limit — left the database
/// exactly as it was. The pass now stops where the provider stops it, keeps the
/// earlier work and says why.
#[tokio::test]
async fn a_stop_from_igdb_does_not_lose_the_matches_already_made() {
    let db = Database::in_memory().await.expect("database");
    let steam = account(&db, StoreId::Steam).await;
    let gog = account(&db, StoreId::Gog).await;

    // "Disco Elysium" goes before "Doom" by title, which is the order in which
    // they come: the first matches by appid and the second stops the pass.
    let exacto = entry(steam, StoreId::Steam, "632470", "Disco Elysium");
    let que_corta = entry(gog, StoreId::Gog, "1234", "Doom");
    StoreEntryRepository(&db)
        .upsert_many(&[exacto.clone(), que_corta.clone()])
        .await
        .expect("write the entries");

    let server = MockServer::start().await;
    // The join goes in batches: the appid goes inside `uid = (…)`, not alone.
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .and(body_string_contains("\"632470\""))
        .respond_with(ResponseTemplate::new(200).set_body_raw(EXTERNAL, "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("where id = 115653"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(GAME_115653, "application/json"))
        .mount(&server)
        .await;
    // The search by title is what meets the limit.
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("search"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    let report = resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("a stop from the provider is a result, not an error");

    assert_eq!(report.linked, 1);
    assert!(
        report
            .stopped
            .as_deref()
            .is_some_and(|reason| reason.contains("limit")),
        "the pass must say why it stopped: {:?}",
        report.stopped
    );

    // And the work before the stop is written, which is all of the point.
    let links = GameLinkRepository(&db).all().await.expect("links");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].store_entry_id, exacto.id);
}

#[tokio::test]
async fn a_game_that_igdb_does_not_know_is_counted_apart_and_gets_no_invented_record() {
    let db = Database::in_memory().await.expect("database");
    let gog = account(&db, StoreId::Gog).await;
    let raro = entry(gog, StoreId::Gog, "9999", "A game that exists in no place");
    StoreEntryRepository(&db)
        .upsert_many(std::slice::from_ref(&raro))
        .await
        .expect("write the entry");

    let server = igdb_server("[]").await;
    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    let report = resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("match");

    assert_eq!(report.unknown, 1);
    assert_eq!(report.linked, 0);
    assert!(
        GameRepository(&db).all().await.expect("records").is_empty(),
        "with no candidates it does not invent a record: the user decides"
    );
}

/// GOG and Epic also have an exact identifier, and since they have it they do
/// not go through the search by title.
///
/// Each store asks for its own source of `external_games`: the `external_id` of
/// Galaxy for GOG and the store offer for Epic, which goes in `raw` because it
/// is not in the copy of the launcher.
#[tokio::test]
async fn gog_and_epic_link_by_identifier_and_never_search_by_title() {
    let db = Database::in_memory().await.expect("database");
    let gog = account(&db, StoreId::Gog).await;
    let epic = account(&db, StoreId::Epic).await;

    let from_gog = entry(gog, StoreId::Gog, "1207658930", "The Witcher 3");
    let mut from_epic = entry(epic, StoreId::Epic, "Heron", "Alan Wake");
    from_epic.raw = serde_json::json!({ "offerId": "OFERTA_ALAN_WAKE" });
    StoreEntryRepository(&db)
        .upsert_many(&[from_gog.clone(), from_epic.clone()])
        .await
        .expect("write the entries");

    let server = MockServer::start().await;
    for (fuente, cuerpo) in [
        (5, r#"[{"id":1,"uid":"1207658930","game":1942}]"#),
        (26, r#"[{"id":2,"uid":"OFERTA_ALAN_WAKE","game":548}]"#),
    ] {
        Mock::given(method("POST"))
            .and(path("/external_games"))
            .and(body_string_contains(format!(
                "external_game_source = {fuente}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_raw(cuerpo, "application/json"))
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/external_games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;

    // If one of the two copies reached the search by title, this expectation of
    // zero calls would fail when the server is dropped.
    Mock::given(method("POST"))
        .and(path("/games"))
        .and(body_string_contains("search"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/games"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"[{"id":1942,"name":"The Witcher 3: Wild Hunt"}]"#,
            "application/json",
        ))
        .mount(&server)
        .await;

    let igdb = IgdbClient::new(reqwest::Client::new())
        .with_bases(server.uri(), format!("{}/token", server.uri()));

    let report = resolve(&db, &igdb, &credentials(), &token(), &Silent)
        .await
        .expect("match");

    assert_eq!(report.linked, 2, "the two copies have an exact identifier");
    assert_eq!(report.review, 0);

    let links = GameLinkRepository(&db).all().await.expect("links");
    assert_eq!(links.len(), 2);
    assert!(
        links.iter().all(|link| link.confidence == 1.0),
        "an external identifier gets no score: it is 1.0 or it is nothing"
    );

    drop(server);
}
