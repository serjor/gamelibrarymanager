# 🎯 To add metadata to a record writes its row again; it does not make a new one

## 💡 Convention

`user_state` — the status, the rating and the notes — is attached to the
`game_id`. From that comes a rule that controls all of the matching:

> When a record that exists gets metadata, **its `GameId` is used again**.

The concrete condition: with no IGDB credentials the application groups the
copies by normalised title and makes a record for them with the title of the
store. When the user configures IGDB later, those records get their metadata. If
that operation made a new record, the `user_state` would stay attached to the old
record that nobody sees: the user would quietly lose what they had written.

These rules come from that:

- `ensure_game` receives the local record to which the copy was already attached
  and **writes that row again**. It does not add a different row.
- If a record with that `igdb_id` already exists, the code links to it. The
  unique index on `game.igdb_id` makes you decide clearly and does not let two
  records collide.
- A record that has no copy behind it gets a logical delete, **but only if it has
  no `user_state`**. With a user status the record stays: a duplicate that you
  see is a nuisance, but data of the user that is lost cannot be recovered.
- A copy that IGDB does not recognise **does not lose its local link**: without
  that rule, a game that the user was already looking at would go out of their
  library.
- None of this deletes rows from the disk: the code marks `deleted_at`.

## 🏆 Benefits

- The user can start to mark their backlog at the first start, with no wait for
  Twitch credentials, and they do not pay for that later.
- The division into four layers gives what it promises: a new match writes
  `game_link`, never `user_state`.
- To use the identifier again makes the operation idempotent. Two matches give
  the same result as one match.
- To keep the orphan records *with* a status turns a possible loss of data into a
  visible duplicate, which the user can correct and, more important, **see**.

## 👀 Examples

### ✅ Good

```rust
/// `local_record` is the record with no metadata to which this copy was already
/// attached, if there was one. The code **uses its identifier again** and does
/// not create a new record, and that is all of the difference: `user_state` is
/// attached to the `game_id`, thus a new record would leave the status that the
/// user wrote with no owner.
async fn ensure_game(/* … */, local_record: Option<GameId>) -> Result<GameId, AppError> {
    let games = GameRepository(db);
    if let Some(existing) = games.find_by_igdb(igdb_id).await? {
        return Ok(existing.id);
    }

    let id = local_record.unwrap_or_default();
    // … the row `id` is written again with the IGDB metadata
}
```

```sql
-- Only the records with nothing of the user behind them get a logical delete.
UPDATE game SET deleted_at = ?, updated_at = ?
 WHERE deleted_at IS NULL
   AND NOT EXISTS (SELECT 1 FROM game_link l WHERE l.game_id = game.id)
   AND NOT EXISTS (SELECT 1 FROM user_state u WHERE u.game_id = game.id)
```

```rust
// With no decision, the local link stays as it was: it is already in `links`
// and `rebuild_auto` will write it again. To remove it would make a game that
// the user already saw go out of the library.
MatchDecision::Review { candidates } => { /* … */ }
```

### ❌ Bad

```rust
// A new record at each addition of metadata: the user_state of the earlier
// record points to a row that nobody shows. The user sees their game with its
// cover and an empty backlog, and there is no way to know what occurred.
let game = Game { id: GameId::new(), igdb_id: Some(meta.igdb_id), /* … */ };
games.upsert(&game).await?;
```

```sql
-- A delete from the disk: it takes the user_state with it and leaves no
-- evidence that the record existed.
DELETE FROM game WHERE id NOT IN (SELECT game_id FROM game_link)
```

```rust
// To merge two records that already have a status and select a "winner".
// Each automatic rule here loses the data of a person.
```

## 🧐 Real world examples

- [`src-tauri/src/identity.rs`](../../src-tauri/src/identity.rs) — `ensure_game`
  with `local_record`, and the `MatchDecision::Review` that keeps the local link.
- [`crates/storage/src/repositories/game.rs`](../../crates/storage/src/repositories/game.rs)
  — `find_local_by_sort_title` (it looks only at records with no `igdb_id`) and
  `soft_delete_orphans` (it obeys `user_state`).
- [`crates/storage/src/repositories/store_entry.rs`](../../crates/storage/src/repositories/store_entry.rs)
  — `pending_metadata`: the copies that you already see but that still wait for
  an identity, with the `manual` links removed.
- [`src-tauri/tests/local_records.rs`](../../src-tauri/tests/local_records.rs) —
  `when_igdb_is_configured_the_record_gets_metadata_and_keeps_the_status`
  examines that the `game_id` is the same before and after, and that the status
  is still there.
- [`migrations/0001_initial.up.sql`](../../migrations/0001_initial.up.sql) — the
  four layers and the unique index that makes you decide and does not let two
  records collide.

## 🔗 Related agreements

- [`README.md`](../../README.md) — the table of crates and the boundary "all of
  the SQL lives in `crates/storage`".
- The plan in `.agents/plans/0001-game-library-manager/plan.html`, phase 2,
  records the decision of the four layers with the alternative refused.
