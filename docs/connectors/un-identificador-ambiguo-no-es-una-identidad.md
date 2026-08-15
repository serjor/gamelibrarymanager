# 🎯 Un identificador ambiguo no es una identidad

## 💡 Convention

Cuando un conector saca de una tienda el identificador con el que esa copia
cruza contra IGDB, **solo lo entrega si la tienda deja claro cuál es**. Si la
respuesta admite dos lecturas, el conector no elige: no entrega identificador y
la copia se empareja por título, que es la vía que ante la duda manda a
revisión.

La regla se aplica en el conector, no en el emparejamiento. Cuando el
identificador llega a `domain::matching`, ya es tarde: `decide_by_external_id`
enlaza con confianza 1.0 y no pregunta nada, porque para eso existe. Quien sabe
si el identificador es dudoso es quien lo ha leído de la tienda.

Ambiguo no quiere decir «no lo he encontrado». Que una copia no tenga
identificador es normal y no cuesta nada: cae en la búsqueda por título. Lo que
esta convención prohíbe es **desempatar por criterios inventados** —el primero
de la lista, el de nombre más corto, el más antiguo— cuando la tienda no dice
cuál corresponde a la copia que el usuario tiene.

## 🏆 Benefits

- Un duplicado visible molesta; una fusión errónea hace perder datos del
  usuario, porque `user_state` cuelga del `game_id`. La regla que gobierna
  `domain::matching` se rompería si el identificador exacto llegara adivinado.
- El coste de la prudencia es una copia en la cola de revisión, y un clic del
  usuario. El coste de acertar por casualidad es que nadie se entera del fallo
  hasta que ya no se puede deshacer.
- Medir cuántas veces la tienda es ambigua convierte la decisión en un número:
  si fueran la mitad de las copias, la vía del identificador no valdría la pena;
  si es una de noventa, sí.

## 👀 Examples

### ✅ Good

Epic vende cada juego por *ofertas*, y el namespace de un juego puede contener
varias. Se entrega la del juego base solo cuando hay exactamente una:

```rust
let mut base = page
    .elements
    .into_iter()
    .filter(|offer| offer.offer_type.as_deref() == Some(OFFER_BASE_GAME));

match (base.next(), base.next()) {
    (Some(unica), None) => Some(unica.id),
    _ => None,
}
```

`Chivalry 2` y `Chivalry 2 Special Edition` viven en el mismo namespace, las dos
son `BASE_GAME` y nada en la respuesta dice cuál posee la cuenta. Sobre 90
namespaces reales pasa una vez; las otras 85 con juego base tienen uno solo.

### ❌ Bad

```rust
// Con dos ediciones en el namespace, esto enlaza la copia del usuario a la
// ficha de la que Epic devolvió primero, que es un orden que nadie promete.
page.elements
    .into_iter()
    .find(|offer| offer.offer_type.as_deref() == Some(OFFER_BASE_GAME))
    .map(|offer| offer.id)
```

Y no falla ruidosamente: enlaza, con confianza 1.0, la edición equivocada. El
usuario ve su juego, escribe su estado encima, y el error solo aparece el día
que se pregunta por qué su partida está en otra ficha.

## 🧐 Real world examples

- [`crates/connectors/src/epic/parse.rs`](../../crates/connectors/src/epic/parse.rs)
  — `parse_base_game_offer` devuelve `None` con cero ofertas base y con dos, y
  lleva la medición con su fecha.
- [`crates/connectors/tests/epic.rs`](../../crates/connectors/tests/epic.rs)
  — `a_copy_carries_the_offer_of_its_namespace` comprueba las dos caras: el
  namespace de una sola oferta la entrega, el de dos no.
- [`src-tauri/src/identity.rs`](../../src-tauri/src/identity.rs) — `external_uid`
  lee `offerId` de `raw`, y su ausencia es exactamente lo que empuja la copia a
  la vía del título.
- [`crates/domain/src/matching.rs`](../../crates/domain/src/matching.rs) —
  `decide_by_external_id` enlaza sin puntuar y sin preguntar, que es el motivo
  por el que lo que le llega tiene que ser seguro.

## 🔗 Related agreements

- [Los endpoints no oficiales se contrastan antes de escribir el conector](contrastar-endpoints-no-oficiales.md)
  — de ahí salen las cifras que sostienen esta convención, y la costumbre de
  fecharlas.
- [Enriquecer una ficha reescribe su fila; no crea otra](../storage/enriquecer-fichas-en-su-sitio.md)
  — explica por qué una fusión equivocada se lleva por delante lo que el usuario
  escribió.
