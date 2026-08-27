//! The two user-facing exports contain the library and no credentials.

use domain::{Game, GameId, PlayStatus, StoreAccount, StoreAccountId, StoreId, UserState};
use gamelibrarymanager_lib::testing::{ExportFormat, export_library_for};
use storage::Database;
use storage::repositories::{
    ConnectorStateRepository, GameRepository, StoreAccountRepository, UserStateRepository,
};
use time::OffsetDateTime;

#[tokio::test]
async fn json_contains_the_library_accounts_and_connector_states() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = Database::in_memory().await.expect("database");
    let account = StoreAccount {
        id: StoreAccountId::new(),
        store: StoreId::Steam,
        account_ref: "76561197960287930".to_owned(),
        display_name: Some("serjor".to_owned()),
        connected_at: OffsetDateTime::now_utc(),
        last_sync_at: None,
    };
    StoreAccountRepository(&db)
        .upsert(&account)
        .await
        .expect("add the account");
    ConnectorStateRepository(&db)
        .set_enabled(StoreId::Epic, false)
        .await
        .expect("save the connector state");

    let game = Game {
        id: GameId::new(),
        canonical_title: "Disco Elysium".to_owned(),
        sort_title: "disco elysium".to_owned(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: vec!["RPG".to_owned()],
    };
    GameRepository(&db)
        .upsert(&game)
        .await
        .expect("add the game");
    UserStateRepository(&db)
        .save(&UserState {
            game_id: game.id,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("at chapter 3".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("save the user state");

    let path = dir.path().join("library.json");
    export_library_for(&db, &path, ExportFormat::Json)
        .await
        .expect("write JSON");
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read JSON"))
            .expect("valid JSON");

    assert_eq!(value["library"][0]["title"], "Disco Elysium");
    assert_eq!(value["library"][0]["notes"], "at chapter 3");
    assert_eq!(value["accounts"][0]["store"], "steam");
    assert_eq!(value["connectors"][0]["store"], "epic");
    assert!(!value.to_string().contains("credential"));
}

#[tokio::test]
async fn csv_contains_only_user_fields_and_escapes_values() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = Database::in_memory().await.expect("database");
    let game = Game {
        id: GameId::new(),
        canonical_title: "A, game".to_owned(),
        sort_title: "a, game".to_owned(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    };
    GameRepository(&db)
        .upsert(&game)
        .await
        .expect("add the game");
    UserStateRepository(&db)
        .save(&UserState {
            game_id: game.id,
            status: Some(PlayStatus::Playing),
            rating: Some(9),
            notes: Some("line 1\nline 2".to_owned()),
            started_at: None,
            finished_at: None,
        })
        .await
        .expect("save the user state");

    let path = dir.path().join("library.csv");
    export_library_for(&db, &path, ExportFormat::Csv)
        .await
        .expect("write CSV");
    let csv = std::fs::read_to_string(path).expect("read CSV");

    assert!(csv.starts_with("game_id,title,status,score,notes\n"));
    assert!(csv.contains("\"A, game\",playing,9,\"line 1\nline 2\"\n"));
    assert!(!csv.contains("summary"));
    assert!(!csv.contains("credential"));
}
