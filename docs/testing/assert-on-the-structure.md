# 🎯 A test asserts on the structure, not on what it looks like

## 💡 Convention

When you can examine something in two ways — with a look at **the shape** of what
the code does, or with a measurement of **the effect** that it produces — assert
on the shape.

In practice that means three things:

- **Count the statements, do not measure their time.** "One query" is a property
  of the code; "less than 500 ms" is a property of the machine that runs it.
- **Assert on the plan of a query, not on the time that it takes.** That it
  starts at the correct table is the reason that it is fast; the time is the
  symptom.
- **Measure the layout, do not look at it.** Whether two boxes cover each other,
  whether a header aligns with its column and whether a text goes out of its box
  are questions with a numeric answer: ask the layout engine, not a screenshot.

The result is not comfortable: **a screenshot is not a test.** It shows something
to a person; it does not decide whether the thing is correct.

## 🏆 Benefits

- **It fails for the reason that it says it watches.** A test with a clock fails
  when the machine compiles something else, and it does not fail when a person
  adds one thousand queries to a local SQLite, because one thousand local queries
  fit easily in one half second. It watched the clock, not the code.
- **The error message is already the diagnosis.** "A subquery starts at
  `store_entry` again" says what to do; "it took 812 ms" says nothing.
- **What you cannot see does not go through.** While the library interface was
  written, three "defects" found by eye in screenshots did not exist — they were
  cell borders and antialiasing — and one that no screenshot showed did exist:
  the covers covered each other by 21 px, because a grid item that stretches
  gives no height to its row. To look gave three false positives and one false
  negative; to measure was correct the four times.
- **It survives changes to the appearance.** A change of a colour or of a font
  size touches none of these tests, because none of them asserts on concrete
  pixels.

## 👀 Examples

### ✅ Good

Count what is really important, which is a property of the code:

```rust
start_counting();
let rows = LibraryRepository(&db).all().await.expect("library");
let made = queries_made();

assert_eq!(
    made, 1,
    "all of the library must come in one query; {made} statements for one \
     thousand games means that somebody added one query for each game"
);
```

Assert on the shape of the plan, which is the cause, and not on the time, which
is the symptom:

```rust
let guilty: Vec<&String> = plan
    .iter()
    .filter(|step| step.contains("store_entry_by_kind"))
    .collect();

assert!(
    guilty.is_empty(),
    "a subquery starts at store_entry again and not at game_link; \
     examine whether a CROSS JOIN was lost:\n{guilty:#?}"
);
```

Ask the browser for the geometry and do not deduce it from an image:

```ts
const boxes = [...document.querySelectorAll(".wall > li")].map((e) =>
  e.getBoundingClientRect(),
);
let overlap = false;
for (let i = 0; i < boxes.length; i++) {
  for (let j = i + 1; j < boxes.length; j++) {
    const a = boxes[i]!;
    const b = boxes[j]!;
    if (a.left < b.right - 0.5 && b.left < a.right - 0.5 &&
        a.top < b.bottom - 0.5 && b.top < a.bottom - 0.5) {
      overlap = true;
    }
  }
}
```

### ❌ Bad

This was in the repository and was removed, because it failed one time in six
with the machine busy and it still would not have found what it said it watched:

```rust
let start = Instant::now();
let rows = LibraryRepository(&db).all().await.expect("library");
assert!(
    start.elapsed() < Duration::from_millis(500),
    "one thousand games must come in less than one half second"
);
```

And its equivalent in the interface, which nobody writes as a test but which
people use as if it were one:

```
// I make a screenshot, I look at it, and I decide that the grid is correct.
```

The two share the same defect: they measure something that depends on conditions
outside the code — the load of the computer, the resolution of the image, the
eyes of the person who looks — thus they do not fail when they must and they are
not correct when something fails.

## 🧐 Real world examples

- [`crates/storage/tests/one_query.rs`](../../crates/storage/tests/one_query.rs)
  counts statements with a `log::Log` of its own, and its header comment explains
  why it stopped to measure their time.
- [`crates/storage/src/repositories/library.rs`](../../crates/storage/src/repositories/library.rs)
  protects with `the_planner_starts_at_game_link` some `CROSS JOIN` clauses that
  look like a mistake: to remove them breaks no result, it only multiplies the
  time of the query by eighty.
- [`test/visual/look.ts`](../../test/visual/look.ts) goes through eight window
  widths and examines overlaps, overflows and the alignment of headers, and it
  measures the contrast of the text against its background in the two themes.
- [`test/visual/harness.ts`](../../test/visual/harness.ts) is what makes that
  possible: it opens the real application in Chromium with the Tauri bridge
  replaced, and it leaves no mock inside `src`.
- [`src/App.test.tsx`](../../src/App.test.tsx) asserts on the checked state of
  the check boxes and on how many times the code wrote, not on how the table
  looks.

## 🔗 Related agreements

- [`AGENTS.md`](../../AGENTS.md) — the index of all of the conventions.
- [`docs/documentation-guidelines.md`](../documentation-guidelines.md) — how to
  write this document.
- The plan of the interface redesign,
  `.agents/plans/0002-rediseno-ui/plan.html`, records in its "Estado" section
  what none of these tools can examine and what still needs a person to open the
  application and look.
