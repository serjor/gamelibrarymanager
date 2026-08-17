//! One long operation at a time.
//!
//! The three long commands — the synchronisation, the prices and the matching —
//! share one cancel flag. Before phase 2 of plan `0004` nothing prevented two of
//! them at the same time: the second cleared the flag of the first, and the
//! first to end cleared the flag under the other.
//!
//! These tests examine the guard and not the commands, because the three
//! commands do the same one line with it: `state.try_begin().ok_or(Busy)?`.

use std::time::Duration;

use gamelibrarymanager_lib::testing::{AppError, AppState, OperationGuard};
use storage::Database;

async fn state(dir: &std::path::Path) -> AppState {
    let db = Database::open(&dir.join("library.db"))
        .await
        .expect("open the database");
    AppState::new(db, dir.join("secrets.bin"))
}

/// The same line that the three long commands write. It is here so that a test
/// cannot examine an answer that the commands do not give.
fn begin(state: &AppState) -> Result<OperationGuard<'_>, AppError> {
    state.try_begin().ok_or(AppError::Busy)
}

#[tokio::test]
async fn a_second_long_operation_is_told_that_the_application_is_busy() {
    let dir = tempfile::tempdir().expect("temporal");
    let state = state(dir.path()).await;

    let first = begin(&state).expect("the first operation takes the right");
    let Err(second) = begin(&state) else {
        panic!("the second long operation must not start");
    };

    assert!(
        matches!(second, AppError::Busy),
        "a second long operation gives Busy: {second}"
    );
    // The user reads this message. It must say what to do.
    assert!(
        second.to_string().contains("cancel"),
        "the message must offer a way out: {second}"
    );

    // And when the first one ends, the next one starts.
    drop(first);
    begin(&state).expect("with the right free, the next operation starts");
}

/// A cancel belongs to the operation that runs, and to no other.
#[tokio::test]
async fn the_next_operation_does_not_inherit_the_cancel_of_the_operation_before() {
    let dir = tempfile::tempdir().expect("temporal");
    let state = state(dir.path()).await;

    let first = begin(&state).expect("the first operation");
    state.cancel_operation();
    assert!(state.operation_cancelled(), "the cancel reaches the first");

    drop(first);
    assert!(
        !state.operation_cancelled(),
        "the guard clears the flag when it goes away"
    );

    let _second = begin(&state).expect("the second operation");
    assert!(
        !state.operation_cancelled(),
        "the second operation starts with no cancel of the first"
    );
}

/// The reason why the guard clears the flag and the command does not.
///
/// A command that goes away — the window closes, the webview goes — never
/// reaches its last line. With `end_operation()` at the end of the command, the
/// flag stayed set and the next operation stopped at its first safe point with
/// nobody who asked for it, and the right stayed taken for ever.
///
/// The future here waits and never ends, and the timeout drops it where it
/// waits, which is what Tauri does with the future of a command that has no
/// window. It measures no time: the limit only makes the drop occur, and the
/// assertions are about the state after it.
#[tokio::test]
async fn a_command_that_goes_away_frees_the_right_and_the_flag() {
    let dir = tempfile::tempdir().expect("temporal");
    let state = state(dir.path()).await;

    let command = async {
        let _guard = begin(&state).expect("the operation takes the right");
        state.cancel_operation();
        std::future::pending::<()>().await;
    };

    let ended = tokio::time::timeout(Duration::from_millis(50), command).await;
    assert!(ended.is_err(), "the command must not end by itself");

    assert!(
        !state.operation_cancelled(),
        "a command that goes away leaves no cancel behind"
    );
    begin(&state).expect("and the next operation starts");
}
