# 🎯 Enriquecer una ficha reescribe su fila; no crea otra

## 💡 Convention

`user_state` —estado, valoración y notas— cuelga del `game_id`. De ahí sale una
regla que gobierna todo el emparejamiento:

> Cuando una ficha existente gana metadatos, **se reutiliza su `GameId`**.

El caso concreto: sin credenciales de IGDB la aplicación agrupa las copias por
título normalizado y les crea una ficha con el título de la tienda. Cuando el
usuario configura IGDB más tarde, esas fichas se enriquecen. Si el
enriquecimiento creara una ficha nueva, el `user_state` se quedaría colgando de
la vieja, que ya nadie ve: el usuario perdería en silencio lo que había escrito.

Reglas que se derivan:

- `ensure_game` recibe la ficha local de la que ya colgaba la copia y **reescribe
  esa fila** en vez de insertar otra.
- Si ya existe una ficha con ese `igdb_id`, se enlaza a ella —el índice único
  sobre `game.igdb_id` obliga a decidirlo explícitamente, no a chocar—.
- Lo que se queda sin ninguna copia detrás se da de baja lógica, **pero solo si
  no tiene `user_state`**. Con estado del usuario se conserva: un duplicado a la
  vista molesta, perder lo que el usuario escribió no se arregla.
- Una copia que IGDB no reconoce **no pierde su enlace local**: seguiría siendo
  un juego que el usuario ya estaba viendo desaparecer de su biblioteca.
- Nada de esto borra filas físicamente: se marca `deleted_at`.

## 🏆 Benefits

- El usuario puede empezar a marcar su backlog desde el primer arranque, sin
  esperar a conseguir credenciales de Twitch, y no paga por ello después.
- La separación en cuatro capas cumple lo que promete: reemparejar reescribe
  `game_link`, nunca `user_state`.
- Reutilizar el identificador hace la operación idempotente. Emparejar dos veces
  da el mismo resultado que emparejar una.
- Conservar las fichas huérfanas *con* estado convierte un posible caso de
  pérdida de datos en un caso de duplicado visible, que el usuario puede
  arreglar y, sobre todo, **ver**.

## 👀 Examples

### ✅ Good

```rust
/// `ficha_local` es la ficha sin metadatos de la que ya colgaba esta copia, si
/// la había. Se **reutiliza su identificador** en vez de crear otra, y esa es
/// toda la diferencia: `user_state` cuelga del `game_id`, así que crear una
/// ficha nueva dejaría huérfano el estado que el usuario ya había escrito.
async fn ensure_game(/* … */, ficha_local: Option<GameId>) -> Result<GameId, AppError> {
    let games = GameRepository(db);
    if let Some(existing) = games.find_by_igdb(igdb_id).await? {
        return Ok(existing.id);
    }

    let id = ficha_local.unwrap_or_default();
    // … se reescribe la fila `id` con los metadatos de IGDB
}
```

```sql
-- Solo se da de baja lo que no tiene nada del usuario detrás.
UPDATE game SET deleted_at = ?, updated_at = ?
 WHERE deleted_at IS NULL
   AND NOT EXISTS (SELECT 1 FROM game_link l WHERE l.game_id = game.id)
   AND NOT EXISTS (SELECT 1 FROM user_state u WHERE u.game_id = game.id)
```

```rust
// Sin decisión, el enlace local que hubiera se queda como estaba: ya está en
// `links` y `rebuild_auto` lo reescribirá igual. Quitarlo haría desaparecer de
// la biblioteca un juego que el usuario ya veía.
MatchDecision::Review { candidates } => { /* … */ }
```

### ❌ Bad

```rust
// Ficha nueva en cada enriquecimiento: el user_state de la anterior se queda
// apuntando a una fila que ya no se enseña. El usuario ve su juego con la
// portada puesta y el backlog en blanco, y no hay forma de saber qué pasó.
let game = Game { id: GameId::new(), igdb_id: Some(meta.igdb_id), /* … */ };
games.upsert(&game).await?;
```

```sql
-- Borrado físico: se lleva por delante el user_state en cascada y no deja
-- rastro de que existió.
DELETE FROM game WHERE id NOT IN (SELECT game_id FROM game_link)
```

```rust
// Fusionar dos fichas que ya tienen estado eligiendo una «ganadora».
// Cualquier criterio automático aquí pierde datos de alguien.
```

## 🧐 Real world examples

- [`src-tauri/src/identity.rs`](../../src-tauri/src/identity.rs) — `ensure_game`
  con `ficha_local`, y el `MatchDecision::Review` que conserva el enlace local.
- [`crates/storage/src/repositories/game.rs`](../../crates/storage/src/repositories/game.rs)
  — `find_local_by_sort_title` (solo mira fichas sin `igdb_id`) y
  `soft_delete_orphans` (respeta `user_state`).
- [`crates/storage/src/repositories/store_entry.rs`](../../crates/storage/src/repositories/store_entry.rs)
  — `pending_metadata`: las copias que ya se ven pero siguen esperando identidad,
  excluyendo los enlaces `manual`.
- [`src-tauri/tests/fichas_locales.rs`](../../src-tauri/tests/fichas_locales.rs) —
  `al_configurar_igdb_la_ficha_se_enriquece_sin_perder_el_estado` comprueba que
  el `game_id` es el mismo antes y después, y que el estado sigue ahí.
- [`migrations/0001_initial.up.sql`](../../migrations/0001_initial.up.sql) — las
  cuatro capas y el índice único que fuerza a decidir en vez de chocar.

## 🔗 Related agreements

- [`README.md`](../../README.md) — la tabla de crates y la frontera «todo el SQL
  vive en `crates/storage`».
- El plan en `.agents/plans/0001-game-library-manager/plan.html`, fase 2, recoge
  la decisión de las cuatro capas con su alternativa descartada.
