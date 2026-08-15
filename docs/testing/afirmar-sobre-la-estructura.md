# 🎯 Una comprobación afirma sobre la estructura, no sobre lo que parece

## 💡 Convention

Cuando algo se pueda comprobar de dos maneras —mirando **la forma** de lo que
hace el código, o midiendo **el efecto** que produce—, se afirma sobre la forma.

En la práctica eso significa tres cosas:

- **Contar sentencias, no cronometrarlas.** «Una consulta» es una propiedad del
  código; «menos de 500 ms» es una propiedad de la máquina que lo ejecuta.
- **Afirmar sobre el plan de una consulta, no sobre lo que tarda.** Que arranque
  por la tabla que toca es la razón de que sea rápida; el tiempo es el síntoma.
- **Medir la maquetación, no mirarla.** Si dos cajas se solapan, si una cabecera
  cuadra con su columna o si un texto se sale de su caja son preguntas con
  respuesta numérica: se le preguntan al motor de maquetación, no a una captura.

El corolario incómodo: **una captura de pantalla no es una comprobación.** Sirve
para enseñarle algo a una persona, no para decidir si está bien.

## 🏆 Benefits

- **Falla por lo que dice que vigila.** Un test cronometrado falla cuando la
  máquina está compilando otra cosa, y no falla cuando alguien mete mil consultas
  a un SQLite local, porque mil consultas locales caben de sobra en medio
  segundo. Vigilaba el reloj, no el código.
- **El mensaje de error ya es el diagnóstico.** «Alguna subconsulta vuelve a
  arrancar por `store_entry`» dice qué hacer; «ha tardado 812 ms» no dice nada.
- **No se puede pasar por alto lo que no se ve.** Al escribir la interfaz de la
  biblioteca, tres «fallos» detectados a ojo en capturas no existían —eran
  bordes de celda y antialiasing— y uno que no se veía en ninguna captura sí:
  las portadas se solapaban 21 px porque un item de rejilla estirado no aporta
  altura a su fila. Mirar dio tres falsos positivos y un falso negativo; medir
  acertó las cuatro veces.
- **Sobrevive a los cambios cosméticos.** Cambiar un color o un tamaño de fuente
  no toca ninguna de estas comprobaciones, porque ninguna afirma sobre píxeles
  concretos.

## 👀 Examples

### ✅ Good

Contar lo que de verdad importa, que es una propiedad del código:

```rust
empezar_a_contar();
let rows = LibraryRepository(&db).all().await.expect("biblioteca");
let hechas = consultas_hechas();

assert_eq!(
    hechas, 1,
    "la biblioteca entera tiene que salir en una consulta; {hechas} sentencias \
     para mil juegos significa que alguien ha metido una consulta por juego"
);
```

Afirmar sobre la forma del plan, que es la causa, y no sobre el tiempo, que es
el síntoma:

```rust
let culpables: Vec<&String> = plan
    .iter()
    .filter(|paso| paso.contains("store_entry_by_kind"))
    .collect();

assert!(
    culpables.is_empty(),
    "alguna subconsulta vuelve a arrancar por store_entry en vez de por \
     game_link; revisa que no se haya perdido un CROSS JOIN:\n{culpables:#?}"
);
```

Preguntarle la geometría al navegador en vez de deducirla de una imagen:

```ts
const cajas = [...document.querySelectorAll(".pared > li")].map((e) =>
  e.getBoundingClientRect(),
);
let solapan = false;
for (let i = 0; i < cajas.length; i++) {
  for (let j = i + 1; j < cajas.length; j++) {
    const a = cajas[i]!;
    const b = cajas[j]!;
    if (a.left < b.right - 0.5 && b.left < a.right - 0.5 &&
        a.top < b.bottom - 0.5 && b.top < a.bottom - 0.5) {
      solapan = true;
    }
  }
}
```

### ❌ Bad

Esto estuvo en el repositorio y se quitó, porque fallaba una de cada seis veces
con la máquina ocupada y aun así no habría cazado lo que decía vigilar:

```rust
let inicio = Instant::now();
let rows = LibraryRepository(&db).all().await.expect("biblioteca");
assert!(
    inicio.elapsed() < Duration::from_millis(500),
    "mil juegos tienen que salir en menos de medio segundo"
);
```

Y su equivalente en la interfaz, que no llega a escribirse como test pero sí se
usa como si lo fuera:

```
// Hago una captura, la miro, y decido que la rejilla está bien.
```

Las dos comparten el mismo defecto: miden algo que depende de cosas ajenas al
código —la carga del ordenador, la resolución de la imagen, la vista de quien
mira— y por eso ni fallan cuando deberían ni aciertan cuando fallan.

## 🧐 Real world examples

- [`crates/storage/tests/una_sola_consulta.rs`](../../crates/storage/tests/una_sola_consulta.rs)
  cuenta sentencias con un `log::Log` propio, y su comentario de cabecera explica
  por qué dejó de cronometrarlas.
- [`crates/storage/src/repositories/library.rs`](../../crates/storage/src/repositories/library.rs)
  guarda con `el_planificador_arranca_por_game_link` unos `CROSS JOIN` que
  parecen un descuido: quitarlos no rompe ningún resultado, solo multiplica por
  ochenta lo que tarda la consulta.
- [`test/visual/mirar.ts`](../../test/visual/mirar.ts) recorre ocho anchos de
  ventana comprobando solapes, desbordes y alineación de cabeceras, y mide el
  contraste del texto contra su fondo en los dos temas.
- [`test/visual/arnes.ts`](../../test/visual/arnes.ts) es lo que lo hace posible:
  abre la aplicación de verdad en Chromium sustituyendo el puente de Tauri, sin
  dejar ningún mock dentro de `src`.
- [`src/App.test.tsx`](../../src/App.test.tsx) afirma sobre el estado marcado de
  las casillas y sobre cuántas veces se ha escrito, no sobre cómo se ve la tabla.

## 🔗 Related agreements

- [`AGENTS.md`](../../AGENTS.md) — índice de todas las convenciones.
- [`docs/documentation-guidelines.md`](../documentation-guidelines.md) — cómo se
  escribe este documento.
- El plan del rediseño,
  `.agents/plans/0002-rediseno-ui/plan.html`, recoge en su sección «Estado» lo
  que **no** puede comprobar ninguna de estas herramientas y sigue necesitando
  que una persona abra la aplicación y mire.
