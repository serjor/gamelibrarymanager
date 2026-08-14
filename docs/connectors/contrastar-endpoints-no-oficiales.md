# 🎯 Los endpoints no oficiales se contrastan antes de escribir el conector

## 💡 Convention

Ninguna de las tiendas que este proyecto lee, salvo Steam, tiene API pública. Lo
que hay es documentación de la comunidad, a menudo de hace años, y lanzarse a
implementar contra ella es escribir código contra endpoints que quizá ya no
existen.

Antes de la primera línea de un conector, dos pasos, en este orden:

1. **Leer la implementación de referencia viva.** Heroic Games Launcher y gogdl
   para GOG, legendary para Epic. Son GPLv3 —compatible— y, sobre todo, están
   mantenidas: su historial de commits dice qué se rompió y cuándo. Un
   `git log` sobre el fichero de autenticación vale más que cualquier wiki.
2. **Probar los endpoints a mano.** Un `curl` por endpoint distingue las tres
   respuestas que importan: `200` vive, `401` vive y pide credenciales, `302` a
   una pantalla de login está muerto.

Solo entonces se implementa, y **contra lo que se ha comprobado**, no contra lo
que decía el plan.

El resultado de la comprobación se escribe **con fecha** en el `//!` del módulo.
Un endpoint no oficial caduca; saber cuándo se miró por última vez es la mitad
del diagnóstico la próxima vez que se rompa.

Si al contrastar resulta que el flujo entero es inviable, se aplica el fallback
acordado y se dice; no se improvisa un camino alternativo sobre la marcha. Que
un endpoint concreto se haya mudado **no** es motivo para el fallback: es motivo
para usar el nuevo.

## 🏆 Benefits

- Se descubre lo que está roto antes de gastar el esfuerzo, no después.
- La fecha en el módulo convierte «esto ya no funciona» en «esto funcionaba el
  14 de agosto de 2026, mira qué ha cambiado desde entonces».
- Leer la implementación de referencia enseña las trampas que la documentación
  no cuenta —qué campos hay que filtrar, cómo se pagina, qué rota al usarse—.
- Al ser todo GPLv3, si hiciera falta portar código se puede, conservando las
  cabeceras y anotándolo en [`NOTICE`](../../NOTICE).

## 👀 Examples

### ✅ Good

Contrastar y anotar, con fecha y con el porqué:

```rust
//! ## Vigencia de los endpoints (comprobado el 2026-08-14)
//!
//! El plan documentaba endpoints de un volcado de 2018 y la mitad ya no vale:
//!
//! - `auth.gog.com/auth` y `auth.gog.com/token` **siguen bien**. El token
//!   responde `invalid_grant` a un código inventado, es decir, acepta el
//!   cliente y solo rechaza el código.
//! - `embed.gog.com/user/data/games` y `embed.gog.com/account/getFilteredProducts`
//!   **están muertos**: responden 302 a la pantalla de login. Heroic los
//!   sustituyó en su PR #5718 (junio de 2026) y ya no le queda ni una
//!   referencia a `embed.gog.com` en su código de biblioteca.
//! - La biblioteca se lee hoy de `galaxy-library.gog.com/users/{id}/releases`,
//!   paginada por `page_token`.
```

La comprobación que sostiene esas tres líneas:

```sh
# 302 a la pantalla de login: muerto.
curl -o /dev/null -w "%{http_code} %{redirect_url}\n" \
  https://embed.gog.com/user/data/games
# 401: vivo, solo pide credenciales.
curl -o /dev/null -w "%{http_code}\n" \
  https://galaxy-library.gog.com/users/1/releases
# invalid_grant, no invalid_client: el par de cliente sigue siendo válido.
curl "https://auth.gog.com/token?client_id=…&client_secret=…&grant_type=authorization_code&code=INVALIDO"
```

Y la trampa que solo se ve leyendo la referencia:

```rust
// `platform_id` importa más de lo que parece: Galaxy también lista lo que el
// usuario tiene en otras tiendas conectadas, así que sin filtrar aquí el
// conector de GOG acabaría inventándose copias de Steam.
.filter(|item| item.owned && item.platform_id == PLATFORM_GOG)
```

### ❌ Bad

```rust
//! Endpoints según gogapidocs.
//! Biblioteca: embed.gog.com/user/data/games
```

Sin fecha, sin comprobar, y contra un endpoint que lleva meses devolviendo una
redirección al login. El conector compila, los tests con fixtures inventadas
pasan, y no funciona nada contra la tienda real.

```rust
// Suponer la forma de la respuesta en vez de mirarla: aquí `id` llega como
// número, y tratarlo como texto sin convertirlo deja el mapa vacío en silencio.
let id: String = product.id;
```

```rust
// «Devuelve 403, probaré a mandar la cookie de sesión del navegador.»
// Improvisar un camino distinto al acordado en cuanto el primero falla.
```

## 🧐 Real world examples

- [`crates/connectors/src/gog/mod.rs`](../../crates/connectors/src/gog/mod.rs) —
  el `//!` con la vigencia fechada y el resultado de cada comprobación.
- [`crates/connectors/src/gog/parse.rs`](../../crates/connectors/src/gog/parse.rs)
  — el filtro por `platform_id` y la lectura del `id` numérico como texto: las
  dos trampas que salieron de leer Heroic, no la documentación.
- [`crates/connectors/tests/gog.rs`](../../crates/connectors/tests/gog.rs) — las
  fixtures se tomaron de respuestas reales del día de la comprobación; el
  encabezado del fichero lo dice.
- [`NOTICE`](../../NOTICE) — registra Heroic y gogdl como origen del
  conocimiento, con sus licencias, aunque no se haya copiado código.
- El mensaje del commit `feat: fase 6` desarrolla por qué **no** se aplicó el
  fallback de importación manual: el flujo funcionaba, solo se había mudado el
  listado.

## 🔗 Related agreements

- [Ninguna credencial de tienda va dentro del binario](credenciales-fuera-del-binario.md)
  — la otra mitad de lo que hace falta para añadir una tienda.
- [Todo enlace de la interfaz necesita alcance explícito en la capacidad](../tauri/alcance-de-urls-en-capacidades.md)
  — comprobar en vez de suponer, aplicado a los permisos.
