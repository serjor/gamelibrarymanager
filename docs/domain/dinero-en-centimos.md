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

Formatting belongs to the interface and to one function, `dinero`, which uses
the currency of the price and the language of the application.

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
/// El precio tal y como llega: con el importe repetido en dos formas.
///
/// Se lee `amountInt`, que son céntimos enteros, y se ignora `amount`, que es el
/// mismo número en coma flotante. La forma que no pierde nada es la entera.
struct RawPrice {
    #[serde(rename = "amountInt")]
    amount_int: i64,
    currency: String,
}
```

And it becomes text once, where it is painted:

```ts
export function dinero(cents: number, currency: string): string {
  return new Intl.NumberFormat("es-ES", { style: "currency", currency }).format(cents / 100);
}
```

### ❌ Bad

```rust
// Los céntimos convertidos a euros en cuanto entran. A partir de aquí ya no se
// puede sumar ni comparar sin arrastrar el error.
struct Price { amount: f64 }
```

```sql
-- Un precio como REAL. La base de datos guarda 19.989999999999998 y el mínimo
-- histórico deja de coincidir consigo mismo.
amount REAL NOT NULL
```

```rust
// Recalcular el descuento. El proveedor dice 60 %, esta cuenta dice 59 %, y en
// la misma fila aparecen los dos números.
let cut = 100 - (price * 100 / regular);
```

```tsx
// Un importe sin su moneda: se pinta en euros porque sí, y quien compra en
// libras ve un precio que no es el suyo.
<span>{(precio.amount / 100).toFixed(2)} €</span>
```

## 🧐 Real world examples

- [`crates/domain/src/prices.rs`](../../crates/domain/src/prices.rs) — `Money`,
  and why it lives in the domain and not in the provider.
- [`crates/metadata/src/itad/parse.rs`](../../crates/metadata/src/itad/parse.rs)
  — the two forms that arrive, and which one is read.
- [`migrations/0007_prices.up.sql`](../../migrations/0007_prices.up.sql) — the
  columns are `INTEGER`, and the currency is next to them.
- [`src/features/wishlist/precios.ts`](../../src/features/wishlist/precios.ts) —
  `dinero` is the only place where an amount becomes text.
- [`src/features/wishlist/precios.test.ts`](../../src/features/wishlist/precios.test.ts)
  — the same amount in two currencies is not written the same way.

## 🔗 Related agreements

- [A price is a cache of somebody else's data, and it is replaced whole](../storage/precios-son-cache-que-se-sustituye.md)
  — where these amounts are kept, and for how long.
- [Ningún componente declara un color: todos salen de los tokens](../ui/tokens-como-unica-fuente-de-color.md)
  — the same shape of rule on the interface side: one source, and no local
  copies.
