# 🎯 One state for the two view modes: the views only show

## 💡 Convention

The library has two presentations — a table and a wall of covers — and **one
state**. The filter, the sort, the selection and the open record live in
[`Library.tsx`](../../src/features/library/Library.tsx). It applies the filter
and the sort **one time** and gives the view in use the result already
calculated.

A view receives what it must show and reports what the user does. It does not
filter, it does not sort and it keeps nothing that the other view also needs:

```tsx
const shared = { rows: visible, selected, onSelect, onOpen, opened };
```

When two views need the same action, the action also goes up: the selection of a
range with the shift key lives in
[`useSelection.ts`](../../src/features/library/useSelection.ts) and not in each
view.

The result in the other direction is this: **a screen that makes its own
divisions is not a view mode**. "Today" does not read the library filters because
it shares no contract with the library; thus it is a tab and not a third button
beside "Table" and "Covers".

## 🏆 Benefits

- **A change of view cannot change which games are in front of you.** And that is
  not a condition to examine at each change: there are not two places where they
  could become different.
- **An action operates in the same way in the two views.** If each view kept its
  own anchor of the range, to start a selection in the table and finish it in the
  covers would give two results for the same `⇧+click`.
- **What is new is written one time.** The bulk edit bar does not know which view
  is below it: it receives the selection and that is all.
- **The view stays small and you can delete it.** `LibraryWall` was written from
  zero in phase 4 with no change to what the table already did.

## 👀 Examples

### ✅ Good

```tsx
// Library.tsx: the filter and the sort are applied here, one time.
const visible = useMemo(
  () => applySort(applyFilters(rows, filters), sort),
  [rows, filters, sort],
);

const shared = { rows: visible, selected, onSelect: mark, onOpen, opened };

return view === "table" ? (
  <LibraryTable {...shared} sort={sort} onSort={setSort} />
) : (
  <LibraryWall {...shared} />
);
```

`sort` goes down to the table because the table is the only view with headers
that sort, but the state stays above: the wall shows the sorted games and does
not know that `sort` exists.

### ❌ Bad

```tsx
// LibraryWall.tsx
export function LibraryWall({ rows, filters }: Props) {
  // Each view filters for itself…
  const visible = useMemo(() => applyFilters(rows, filters), [rows, filters]);
  // …and keeps its own selection.
  const [selected, setSelected] = useState<Set<string>>(new Set());
```

Now there are two implementations of the same filter, and on the day that one of
them gets a new condition — accents, wished-for games, anything — the other stays
behind and nothing fails. Also, to mark four games in the table and change to the
covers loses them, thus the bulk bar shows a different thing and the path that
you took decides which.

## 🧐 Real world examples

- [`src/features/library/Library.tsx`](../../src/features/library/Library.tsx)
  has all of the state: `filters`, `sort`, `selected` and `opened`. It also has
  the rule that a change of the filter empties the selection, so that the batch
  does not write on games that you no longer see.
- [`src/features/library/useSelection.ts`](../../src/features/library/useSelection.ts)
  keeps the anchor of the range out of the two views, with the reason written.
- [`src/features/today/Today.tsx`](../../src/features/today/Today.tsx) is the
  opposite condition, and thus it is a tab: it calculates its own divisions on
  `rows` and does not receive `filters`.
- [`src/App.test.tsx`](../../src/App.test.tsx) — "a change of view does not
  change which games are in front of you" and "what is selected in the table
  stays selected in the covers" are the tests of this rule.

## 🔗 Related agreements

- [No component declares a colour](tokens-as-the-only-source-of-colour.md) — the
  same idea applied to colour: one source, many users.
- The plan `.agents/plans/0002-rediseno-ui/plan.html` records why "Today" is not
  a view mode, with the alternative refused.
- [`AGENTS.md`](../../AGENTS.md) — the index of all of the conventions.
