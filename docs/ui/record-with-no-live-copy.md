# 🎯 A record with no live copy stays visible

## 💡 Convention

A record with no owned store and no wishlist store stays in the library.
The interface marks it as `No longer in a store`.
The record keeps the status, the rating and the notes of the user.

"Today" does not propose this record.
The record is not a wishlist item because no store has it on the wishlist.

## 🏆 Benefits

- The user can find a record after a store removes the game.
- The user does not lose the notes and the status.
- The library does not offer a game that the user cannot start from a store.
- The filter gives the user a direct list of these records.

## 👀 Examples

### ✅ Good

```ts
export function isNoLongerInStore(row: LibraryRow): boolean {
  return row.owned_stores.length === 0 && row.wishlist_stores.length === 0;
}
```

```tsx
{isNoLongerInStore(row) ? (
  <span className="status gone">No longer in a store</span>
) : (
  <span className="hint">{row.owned_stores.join(" · ")}</span>
)}
```

### ❌ Bad

```ts
// A record with no owned copy is always a wishlist record.
if (row.owned_stores.length === 0) return "only in the wishlist";
```

This code hides a record after every store removes its copy.
It also gives the record the wrong meaning.

## 🧐 Real world examples

- [`src/features/library/filters.ts`](../../src/features/library/filters.ts)
  defines the rule and applies the availability filter.
- [`src/features/library/LibraryTable.tsx`](../../src/features/library/LibraryTable.tsx)
  marks the record in the table.
- [`src/features/library/LibraryWall.tsx`](../../src/features/library/LibraryWall.tsx)
  marks the same record in the wall of covers.
- [`src/features/game/GameDetail.tsx`](../../src/features/game/GameDetail.tsx)
  shows the state in the record.
- [`src/features/today/shelves.ts`](../../src/features/today/shelves.ts)
  excludes records with no live owned copy.
- [`src/features/library/filters.test.ts`](../../src/features/library/filters.test.ts)
  tests the filter and the distinction from a wishlist record.
- [`src/features/today/shelves.test.ts`](../../src/features/today/shelves.test.ts)
  tests that "Today" does not propose the record.

## 🔗 Related agreements

- [One state for the two view modes](one-state-for-the-two-view-modes.md) — the
  table and the wall receive the same filtered rows.
- [No component declares a colour](tokens-as-the-only-source-of-colour.md) —
  the marker uses a palette token and does not declare a literal color.
- [`AGENTS.md`](../../AGENTS.md) — the index of the repository conventions.
