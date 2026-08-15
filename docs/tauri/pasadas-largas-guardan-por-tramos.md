# 🎯 A long pass saves as it goes, and a provider that cuts it off is a result

## 💡 Convention

Three of the use cases of this application walk over hundreds of items and talk
to somebody else's server on every one: the synchronisation, the matching and
the prices. All three take minutes, and all three will be interrupted. What they
must never do is throw away the work they had already done.

Four rules, and they are one rule seen from four sides:

1. **Write in chunks, not at the end.** A pass that only writes on its last line
   is a pass that loses everything to one 429. The matching writes every 25
   games; the synchronisation writes every account; the prices write every game.
2. **A provider that cuts you off is a result, not an error.** The pass stops
   where it was cut, keeps what it had, and returns the reason inside the
   report. What does rise as an error is a failure that stops the writing
   itself, which is the database: if nothing can be written, there is nothing to
   save.
3. **Say why it stopped.** A report nobody paints is a report that does not
   exist. A half done pass with no explanation is worse than an error, because
   the user cannot tell it apart from a pass that had nothing to do.
4. **Pressing again continues.** Every pass is idempotent and reads its pending
   work from the database, so it never needs to remember where it was.

## 🏆 Benefits

- Five minutes of a rate limited provider stop costing five minutes of work.
- The user gets a way forward that is one click, instead of a run that has to
  start from zero and will hit the same limit at the same place.
- Cancelling becomes cheap, so a slow pass can be interrupted without a price,
  and that is what makes the cancel button honest.
- A crash, a closed window or a kill leaves the same state as a cancel: the last
  chunk, and nothing half written.

## 👀 Examples

### ✅ Good

The provider stops the pass; the database does not:

```rust
let decision = match decide(igdb, credentials, token, &entry).await {
    Ok(decision) => decision,
    // Un corte del proveedor para la pasada aquí mismo, y lo de atrás se
    // guarda igual. Un fallo de la base de datos sí sube: si no se puede
    // escribir, no hay nada que salvar.
    Err(AppError::Metadata(error)) => {
        report.stopped = Some(error.to_string());
        break;
    }
    Err(otro) => return Err(otro),
};
```

The chunk, with the reason it can be repeated:

```rust
desde_el_ultimo_guardado += 1;
if desde_el_ultimo_guardado == TRAMO {
    // `rebuild_auto` reescribe el mismo conjunto de enlaces cada vez, así que
    // llamarlo veinte veces deja lo mismo que llamarlo una.
    GameLinkRepository(db).rebuild_auto(&links).await?;
    desde_el_ultimo_guardado = 0;
}
```

And the interface says it out loud:

```tsx
if (stopped !== null) {
  setError(
    `El emparejamiento se paró: ${stopped}. Lo hecho hasta ahí está ` +
      "guardado; vuelve a pulsar «Emparejar» para seguir desde donde iba.",
  );
}
```

### ❌ Bad

```rust
// Escribir solo al final. Un 429 en el juego trescientos deja la base
// exactamente como estaba, después de cinco minutos de espera.
for entry in pending {
    let decision = decide(igdb, credentials, token, &entry).await?;
    links.push(/* … */);
}
GameLinkRepository(db).rebuild_auto(&links).await?;
```

```rust
// Tragarse el motivo. La pasada devuelve cero emparejados y el usuario no
// distingue «no había nada que hacer» de «IGDB me cortó».
Err(_) => break,
```

```rust
// Tratar un fallo de la base de datos como un corte del proveedor. Se sigue
// como si nada, escribiendo en algo que no acepta escrituras.
Err(_) => { report.stopped = Some(error.to_string()); break; }
```

```rust
// Guardar cada elemento en su propia transacción «por si acaso». Mil
// transacciones donde caben cuarenta: el tramo existe para eso.
for entry in pending {
    GameLinkRepository(db).rebuild_auto(&links).await?;
}
```

## 🧐 Real world examples

- [`src-tauri/src/identity.rs`](../../src-tauri/src/identity.rs) — `TRAMO`, y el
  `match` que separa un corte del proveedor de un fallo de la base de datos.
- [`src-tauri/tests/identity.rs`](../../src-tauri/tests/identity.rs) — un 429 a
  mitad deja escrito lo de antes del corte, con su motivo.
- [`src-tauri/src/sync.rs`](../../src-tauri/src/sync.rs) — el mismo reparto por
  cuentas: una tienda que falla es una línea en `failures`, no el final de la
  pasada.
- [`src-tauri/src/prices.rs`](../../src-tauri/src/prices.rs) — lo caro es
  identificar cada deseado, y eso se anota juego a juego; los precios de un lote
  que falle los rehace la siguiente pasada sin volver a buscar a nadie.
- [`src/App.tsx`](../../src/App.tsx) — el aviso que convierte el motivo en algo
  que el usuario lee.

## 🔗 Related agreements

- [Every store connector has a switch of its own](../connectors/switch-per-connector.md)
  — la misma idea entre tiendas: lo que se rompe, se rompe solo.
- [A price is a cache of somebody else's data, and it is replaced whole](../storage/precios-son-cache-que-se-sustituye.md)
  — por qué cancelar a mitad de los precios no borra nada que siga valiendo.
