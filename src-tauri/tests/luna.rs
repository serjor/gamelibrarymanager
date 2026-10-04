//! The complete cycle of Luna against a pretend Luna and a real database: to
//! switch it on, to see a wish marked, to see the mark go when the catalogue
//! rotates, and to lose nothing when Luna fails.

use domain::{Game, GameId, PlatformFamily};
use gamelibrarymanager_lib::testing::{
    AppError, ExportFormat, WishTarget, add_manual_wish_for, disable_luna, enable_luna,
    export_library_for, refresh_luna,
};
use metadata::LunaClient;
use storage::Database;
use storage::repositories::{LibraryRepository, LunaRepository};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// One tile in the shape that Luna gives on 2026-10-04: `presentationData` is
/// JSON inside a string, and the page of the game is in the action.
fn tile(product_id: &str, title: &str, target: &str) -> serde_json::Value {
    serde_json::json!({
        "id": format!("game_tile_{product_id}"),
        "type": "GAME_TILE",
        "presentationData": serde_json::json!({
            "gameId": product_id,
            "title": title,
            "hoverDetails": {
                "productUrl": "https://play.amazon.es/play?r=web&asin=B000000000&territory=ES"
            }
        }).to_string(),
        "actions": [{ "target": target }]
    })
}

fn page(tiles: Vec<serde_json::Value>) -> String {
    serde_json::json!({
        "pageContext": { "pageType": "browse_results_included_with_prime" },
        "pageMemberGroups": {
            "mainContent": { "widgets": [{ "type": "GRID", "widgets": tiles }] }
        }
    })
    .to_string()
}

fn hogwarts() -> serde_json::Value {
    tile(
        "amzn1.adg.product.h",
        "Hogwarts Legacy",
        "/game/hogwarts-legacy/B0CMQK7H1T",
    )
}

fn fortnite() -> serde_json::Value {
    tile("amzn1.luna.product.f", "Fortnite", "/game/fortnite")
}

/// A Luna that answers `body` with `status`, once for each mount.
async fn luna_answers(server: &MockServer, status: u16, body: String) {
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/getPage"))
        .respond_with(ResponseTemplate::new(status).set_body_raw(body, "application/json"))
        .mount(server)
        .await;
}

fn client(server: &MockServer) -> LunaClient {
    LunaClient::new(reqwest::Client::new()).with_base(server.uri())
}

/// A game that the user wished by hand, for a PC.
async fn wish(db: &Database, title: &str) -> GameId {
    let game = Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    add_manual_wish_for(db, WishTarget::New(game), PlatformFamily::Pc, "")
        .await
        .expect("wish")
        .game_id
}

async fn luna_url(db: &Database, id: GameId) -> Option<String> {
    LibraryRepository(db)
        .one(id)
        .await
        .expect("row")
        .expect("visible")
        .luna
        .map(|mark| mark.url)
}

#[tokio::test]
async fn a_wish_is_marked_and_the_mark_goes_when_the_catalogue_rotates() {
    let db = Database::in_memory().await.expect("database");
    let server = MockServer::start().await;
    let hogwarts_wish = wish(&db, "Hogwarts Legacy").await;

    luna_answers(&server, 200, page(vec![hogwarts(), fortnite()])).await;
    let report = enable_luna(&db, &client(&server), "es")
        .await
        .expect("switch on");
    assert_eq!(report.games, 2);
    assert_eq!(report.territory.as_deref(), Some("ES"));
    assert_eq!(
        luna_url(&db, hogwarts_wish).await.as_deref(),
        Some("https://luna.amazon.es/game/hogwarts-legacy/B0CMQK7H1T")
    );

    // A wish added after the refresh: no new request is necessary.
    let fortnite_wish = wish(&db, "Fortnite").await;
    assert_eq!(
        luna_url(&db, fortnite_wish).await.as_deref(),
        Some("https://luna.amazon.es/game/fortnite")
    );

    // The month changes and Hogwarts Legacy leaves the catalogue.
    luna_answers(&server, 200, page(vec![fortnite()])).await;
    refresh_luna(&db, &client(&server)).await.expect("refresh");
    assert_eq!(luna_url(&db, hogwarts_wish).await, None);
    assert!(luna_url(&db, fortnite_wish).await.is_some());
}

#[tokio::test]
async fn a_failure_of_luna_keeps_the_last_catalogue() {
    let db = Database::in_memory().await.expect("database");
    let server = MockServer::start().await;
    let id = wish(&db, "Hogwarts Legacy").await;
    luna_answers(&server, 200, page(vec![hogwarts()])).await;
    enable_luna(&db, &client(&server), "ES")
        .await
        .expect("switch on");

    luna_answers(&server, 500, r#"{"message":"unexpected"}"#.to_owned()).await;
    assert!(refresh_luna(&db, &client(&server)).await.is_err());
    assert!(luna_url(&db, id).await.is_some());

    // A page whose shape changed gives no games. That is not a Luna with no
    // games, and the marks stay.
    luna_answers(&server, 200, page(Vec::new())).await;
    assert!(refresh_luna(&db, &client(&server)).await.is_err());
    assert!(luna_url(&db, id).await.is_some());
}

#[tokio::test]
async fn a_country_that_fails_is_not_kept() {
    let db = Database::in_memory().await.expect("database");
    let server = MockServer::start().await;

    luna_answers(&server, 500, r#"{"message":"unexpected"}"#.to_owned()).await;
    assert!(enable_luna(&db, &client(&server), "ES").await.is_err());
    assert_eq!(
        LunaRepository(&db).settings().await.expect("settings"),
        None
    );

    // A country that nobody examined is refused before any request.
    assert!(matches!(
        enable_luna(&db, &client(&server), "ZZ").await,
        Err(AppError::Message(_))
    ));
    assert_eq!(
        LunaRepository(&db).settings().await.expect("settings"),
        None
    );
}

#[tokio::test]
async fn switching_luna_off_removes_every_mark() {
    let db = Database::in_memory().await.expect("database");
    let server = MockServer::start().await;
    let id = wish(&db, "Hogwarts Legacy").await;
    luna_answers(&server, 200, page(vec![hogwarts()])).await;
    enable_luna(&db, &client(&server), "ES")
        .await
        .expect("switch on");

    disable_luna(&db).await.expect("switch off");

    assert_eq!(luna_url(&db, id).await, None);
    // With Luna off a refresh says so, and it does not call Luna.
    assert!(refresh_luna(&db, &client(&server)).await.is_err());
}

#[tokio::test]
async fn the_export_is_the_same_with_luna_on() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = Database::in_memory().await.expect("database");
    let server = MockServer::start().await;
    wish(&db, "Hogwarts Legacy").await;

    let before = dir.path().join("before.json");
    export_library_for(&db, &before, ExportFormat::Json)
        .await
        .expect("export");

    luna_answers(&server, 200, page(vec![hogwarts()])).await;
    enable_luna(&db, &client(&server), "ES")
        .await
        .expect("switch on");
    let after = dir.path().join("after.json");
    export_library_for(&db, &after, ExportFormat::Json)
        .await
        .expect("export");

    assert_eq!(
        std::fs::read_to_string(before).expect("read"),
        std::fs::read_to_string(after).expect("read")
    );
}
