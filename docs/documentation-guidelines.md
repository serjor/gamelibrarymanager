# 🎯 How to write the documentation of this repository

## 💡 Convention

Each convention lives in **a file of its own** in `docs/<area>/`. The areas are
the areas of the repository, not general categories:

```
docs/
  connectors/  stores: endpoints, credentials, authentication
  domain/      pure rules: matching, normalisation
  storage/     schema, layers, migrations
  tauri/       capabilities, commands, windows
  ui/          React, first-start screens
  testing/     fixtures, guard tests
```

Each document has these sections, in this order and with these emoji:

```markdown
# 🎯 The name of the convention

## 💡 Convention
## 🏆 Benefits
## 👀 Examples   (with the subsections ✅ Good and ❌ Bad)
## 🧐 Real world examples
## 🔗 Related agreements
```

These rules are not negotiable:

- **One convention in each file.** If a document explains two things, they are
  two documents.
- **The examples are real code**, with a ✅ and a ❌. The ❌ is the example that
  teaches: without it, the convention looks like a preference.
- **"Real world examples" points to files of this repository**, with a path and
  a line. An invented example becomes old and nobody sees it; an example that
  points to the code breaks where you can see it.
- **Give the reason, not the operation.** The code already gives the operation.
- **Write in English, and use ASD-STE100.** Use short sentences, the active
  voice, one instruction in each sentence, and simple words. This applies to all
  of the project.
- Add each new document to the index of [`AGENTS.md`](../AGENTS.md).

## 🏆 Benefits

- One convention in each file is a convention that you can link to, discuss and
  delete separately. A document that holds everything is never updated, because
  a change to it causes fear.
- Areas that agree with the crates make it clear where to look and where to
  write: they are the same boundaries that the architecture already applies.
- A fixed structure makes the fifth document as easy to read as the first.
- Links to real files make the documentation something that you can examine and
  not a promise.

## 👀 Examples

### ✅ Good

Written from `docs/tauri/`, which is why the path starts with `../../` to reach
the root of the repository:

```markdown
## 🧐 Real world examples

- [`src-tauri/capabilities/default.json`](../../src-tauri/capabilities/default.json)
  lists the four addresses of the setup screens.
- [`src-tauri/tests/capabilities.rs`](../../src-tauri/tests/capabilities.rs)
  fails if one of them stops being permitted.
```

### ❌ Bad

```markdown
## 🧐 Real world examples

- The capability file has the list of permitted URLs.
- There is a test that examines it.
```

With no path you cannot go and look, and when the file moves nobody sees that
the document is now false.

## 🔗 Related agreements

- [`AGENTS.md`](../AGENTS.md) — the index of all of the conventions.
- [`README.md`](../README.md) — the architecture and the crates, which is where
  the areas come from.
- The plan in `.agents/plans/0001-game-library-manager/plan.html` records the
  closed decisions with the alternatives refused. The documentation develops
  those decisions; it does not discuss them again.
