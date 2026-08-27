//! A migration gets a complete SQLite copy before it changes the user's file.
//!
//! The copy must be usable on its own, and repeated attempts must not leave an
//! unbounded number of copies beside the database.

use std::path::{Path, PathBuf};

use storage::Database;
use storage::repositories::GameRepository;

fn backups(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read the temporary directory")
        .map(|entry| entry.expect("read a directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("library.db.bak-"))
        })
        .collect();
    paths.sort();
    paths
}

#[tokio::test]
async fn pending_migrations_leave_a_copy_that_opens_and_keep_three_copies() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = dir.path().join("library.db");

    // Four old-schema openings make four copies. The first copy uses the bare
    // version name; later attempts use a timestamp because that name exists.
    for _ in 0..4 {
        let db = Database::open(&path).await.expect("open the database");
        db.undo_all().await.expect("make an old schema");
        drop(db);
    }

    let copies = backups(dir.path());
    assert_eq!(copies.len(), 3, "only the three newest copies remain");

    let copy = Database::open(copies.last().expect("a backup exists"))
        .await
        .expect("the backup opens");
    assert!(
        GameRepository(&copy)
            .all()
            .await
            .expect("the copied schema is usable")
            .is_empty()
    );
}
