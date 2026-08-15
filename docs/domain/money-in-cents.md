# 🎯 Money is kept in whole cents, and formatted only when it is painted

## 💡 Convention

An amount of money travels through the application as an integer number of the
smallest unit of its currency, with the currency next to it. It becomes text at
the last possible moment: in the component that paints it.

That means:

- `Money { cents: i64, currency: String }` in the domain, `INTEGER` in SQLite,
  `number` in the interface. No `f64`, no `REAL`, no `Decimal`.
- The currency travels with the amount, always. Two amounts of different
  currencies are not comparable, and an amount without its currency is a number.
- The provider gives the same amount twice —`amount` in floating point and
  `amountInt` in cents—. The integer one is read and the other one is ignored.
- What the provider computes, the provider keeps. The discount arrives already
  calculated and is not recomputed: two different roundings of the same discount
  contradict each other on the same screen.

Formatting belongs to the interface and to one function, `money`, which uses the
currency of the price and the language of the application.

## 🏆 Benefits

- 19,99 keeps being 19,99. In floating point it stops being that as soon as two
  amounts are added, and the first place the error shows up is a comparison
  against a historical low, which is the whole point of the wish list screen.
- Comparing, ordering and summing prices is integer arithmetic, exact and cheap.
- A price with its currency cannot be painted in the wrong one by accident: the
  formatter has no default currency to fall back to.
- One formatting function means one place to change the day the application
  speaks another language.

## 👀 Examples

### ✅ Good

The integer form is the one that is read:

```rust
/// The price as it comes: with the quantity given in two forms.
///
/// This code reads `amountInt`, which is whole cents, and ignores `amount`,
/// which is the same number in floating point. The whole form is the form that
/// loses nothing.
struct RawPrice {
    #[serde(rename = "amountInt")]
    amount_int: i64,
    currency: String,
}
```

And it becomes text once, where it is painted:

```ts
export function money(cents: number, currency: string): string {
  return new Intl.NumberFormat("en-GB", { style: "currency", currency }).format(cents / 100);
}
```

### ❌ Bad

```rust
// The cents changed to euros as soon as they come in. From here you can no
// longer add or compare without you carry the error.
struct Price { amount: f64 }
```

```sql
-- A price as REAL. The database keeps 19.989999999999998 and the all-time low
-- stops being equal to itself.
amount REAL NOT NULL
```

```rust
// To calculate the discount again. The provider says 60 %, this calculation
// says 59 %, and the two numbers appear in the same row.
let cut = 100 - (price * 100 / regular);
```

```tsx
// A quantity with no currency: it is shown in euros for no reason, and a user
// who buys in pounds sees a price that is not their price.
<span>{(price.amount / 100).toFixed(2)} €</span>
```

## 🧐 Real world examples

- [`crates/domain/src/prices.rs`](../../crates/domain/src/prices.rs) — `Money`,
  and why it lives in the domain and not in the provider.
- [`crates/metadata/src/itad/parse.rs`](../../crates/metadata/src/itad/parse.rs)
  — the two forms that arrive, and which one is read.
- [`migrations/0007_prices.up.sql`](../../migrations/0007_prices.up.sql) — the
  columns are `INTEGER`, and the currency is next to them.
- [`src/features/wishlist/prices.ts`](../../src/features/wishlist/prices.ts) —
  `money` is the only place where an amount becomes text.
- [`src/features/wishlist/prices.test.ts`](../../src/features/wishlist/prices.test.ts)
  — the same amount in two currencies is not written the same way.

## 🔗 Related agreements

- [A price is a cache of the data of another person, and it is replaced complete](../storage/prices-are-a-cache-that-is-replaced.md)
  — where these amounts are kept, and for how long.
- [No component declares a colour: all of them come from the tokens](../ui/tokens-as-the-only-source-of-colour.md)
  — the same shape of rule on the interface side: one source, and no local
  copies.
