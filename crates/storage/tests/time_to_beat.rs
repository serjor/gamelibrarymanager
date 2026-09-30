//! The cache of the durations: which records a pass asks for, and how the
//! answer reaches the row of the library.

use std::collections::HashMap;

use domain::{Game, GameId, TimeToBeat};
use storage::Database;
use storage::repositories::{GameRepository, LibraryRepository, TimeToBeatRepository};
use time::{Duration, OffsetDateTime};

fn game(title: &str, igdb_id: Option<i64>) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: Vec::new(),
    }
}

fn disco() -> TimeToBeat {
    TimeToBeat {
        hastily: Some(79_200),
        normally: Some(115_200),
        completely: Some(172_800),
        submissions: 412,
    }
}

#[tokio::test]
async fn only_the_records_with_an_igdb_identity_are_due() {
    let db = Database::in_memory().await.expect("database");
    let games = GameRepository(&db);
    games
        .upsert(&game("Disco Elysium", Some(115_653)))
        .await
        .expect("record");
    games
        .upsert(&game("From the title of the store", None))
        .await
        .expect("record");

    let due = TimeToBeatRepository(&db)
        .due(OffsetDateTime::now_utc())
        .await
        .expect("due");

    assert_eq!(due, vec![115_653]);
}

#[tokio::test]
async fn an_answer_reaches_the_row_of_the_library() {
    let db = Database::in_memory().await.expect("database");
    let record = game("Disco Elysium", Some(115_653));
    GameRepository(&db).upsert(&record).await.expect("record");

    let found = HashMap::from([(115_653, disco())]);
    TimeToBeatRepository(&db)
        .save(&[115_653], &found, OffsetDateTime::now_utc())
        .await
        .expect("save");

    let row = LibraryRepository(&db)
        .one(record.id)
        .await
        .expect("query")
        .expect("the row exists");
    assert_eq!(row.time_to_beat, Some(disco()));
}

/// A record that was asked and got no duration is written down, so that the
/// next pass does not ask again at once; and the row of the library shows
/// nothing, not a block of zeros.
#[tokio::test]
async fn a_record_with_no_answer_is_not_due_again_until_it_is_stale() {
    let db = Database::in_memory().await.expect("database");
    let record = game("Nobody played this", Some(9_999));
    GameRepository(&db).upsert(&record).await.expect("record");
    let times = TimeToBeatRepository(&db);

    let asked_at = OffsetDateTime::now_utc();
    times
        .save(&[9_999], &HashMap::new(), asked_at)
        .await
        .expect("save");

    assert!(
        times
            .due(asked_at - Duration::days(30))
            .await
            .expect("due")
            .is_empty()
    );
    assert_eq!(
        times
            .due(asked_at + Duration::seconds(1))
            .await
            .expect("due"),
        vec![9_999]
    );

    let row = LibraryRepository(&db)
        .one(record.id)
        .await
        .expect("query")
        .expect("the row exists");
    assert_eq!(row.time_to_beat, None);
}

/// A cache is replaced complete: a duration that IGDB no longer gives does not
/// stay from the earlier answer.
#[tokio::test]
async fn a_new_answer_replaces_the_row_complete() {
    let db = Database::in_memory().await.expect("database");
    let record = game("Disco Elysium", Some(115_653));
    GameRepository(&db).upsert(&record).await.expect("record");
    let times = TimeToBeatRepository(&db);

    let now = OffsetDateTime::now_utc();
    times
        .save(&[115_653], &HashMap::from([(115_653, disco())]), now)
        .await
        .expect("first answer");
    let later = TimeToBeat {
        hastily: Some(80_000),
        normally: None,
        completely: None,
        submissions: 500,
    };
    times
        .save(&[115_653], &HashMap::from([(115_653, later)]), now)
        .await
        .expect("second answer");

    let row = LibraryRepository(&db)
        .one(record.id)
        .await
        .expect("query")
        .expect("the row exists");
    assert_eq!(row.time_to_beat, Some(later));
}
