# 🎯 An ambiguous identifier is not an identity

## 💡 Convention

When a connector takes from a store the identifier with which that copy joins to
IGDB, it **gives the identifier only if the store makes it clear**. If the answer
permits two readings, the connector does not select: it gives no identifier and
the copy matches by title, which is the path that sends to review when there is
doubt.

The rule applies in the connector, not in the matching. When the identifier
reaches `domain::matching`, it is too late: `decide_by_external_id` links with
confidence 1.0 and asks nothing, because that is its purpose. The code that
knows whether the identifier has doubt is the code that read it from the store.

Ambiguous does not mean "I did not find it". That a copy has no identifier is
usual and costs nothing: it falls to the search by title. What this convention
prohibits is **to break the tie with invented rules** — the first of the list,
the shortest name, the oldest — when the store does not say which one belongs to
the copy that the user has.

## 🏆 Benefits

- A duplicate that you see is a nuisance; an incorrect merge loses data of the
  user, because `user_state` is attached to the `game_id`. The rule that controls
  `domain::matching` would break if the exact identifier were a guess.
- The cost of care is one copy in the review queue, and one click of the user.
  The cost of a correct guess is that nobody sees the defect until you can no
  longer undo it.
- To measure how many times the store is ambiguous turns the decision into a
  number: if it were one half of the copies, the identifier path would not be
  worth the work; if it is one of ninety, it is.

## 👀 Examples

### ✅ Good

Epic sells each game through *offers*, and the namespace of a game can contain
more than one. The code gives the offer of the base game only when there is
exactly one:

```rust
let mut base = page
    .elements
    .into_iter()
    .filter(|offer| offer.offer_type.as_deref() == Some(OFFER_BASE_GAME));

match (base.next(), base.next()) {
    (Some(only), None) => Some(only.id),
    _ => None,
}
```

`Chivalry 2` and `Chivalry 2 Special Edition` live in the same namespace, the two
are `BASE_GAME` and nothing in the answer says which one the account owns. Over
90 real namespaces this occurs one time; the other 85 with a base game have one
only.

### ❌ Bad

```rust
// With two editions in the namespace, this links the copy of the user to the
// record that Epic gave back first, and nobody promises that order.
page.elements
    .into_iter()
    .find(|offer| offer.offer_type.as_deref() == Some(OFFER_BASE_GAME))
    .map(|offer| offer.id)
```

And it does not fail loudly: it links the incorrect edition, with confidence 1.0.
The user sees their game, writes their status on it, and the error appears only
on the day that they ask why their play time is in a different record.

## 🧐 Real world examples

- [`crates/connectors/src/epic/parse.rs`](../../crates/connectors/src/epic/parse.rs)
  — `parse_base_game_offer` gives back `None` with zero base offers and with two,
  and it carries the measurement with its date.
- [`crates/connectors/tests/epic.rs`](../../crates/connectors/tests/epic.rs)
  — `a_copy_carries_the_offer_of_its_namespace` examines the two sides: the
  namespace with one offer gives it, the namespace with two does not.
- [`src-tauri/src/identity.rs`](../../src-tauri/src/identity.rs) — `external_uid`
  reads `offerId` from `raw`, and its absence is exactly what pushes the copy to
  the path of the title.
- [`crates/domain/src/matching.rs`](../../crates/domain/src/matching.rs) —
  `decide_by_external_id` links with no score and with no question, which is the
  reason that what reaches it must be certain.

## 🔗 Related agreements

- [Verify the unofficial endpoints before you write the connector](verify-unofficial-endpoints.md)
  — the numbers that support this convention come from there, together with the
  practice of a date on them.
- [To add metadata to a record writes its row again; it does not make a new one](../storage/enrich-records-in-place.md)
  — it explains why an incorrect merge deletes what the user wrote.
