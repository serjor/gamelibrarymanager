# 🎯 Se desplaza una sola región, y llega a los bordes de la ventana

## 💡 Convention

La ventana es el marco. Dentro hay **exactamente una** caja que se desplaza —la
lista, la cola o «Hoy»—, y esa caja **llega a los dos bordes** de la ventana.

De ahí salen tres reglas concretas:

- **La altura se reparte, no se inventa.** Nada de `height: 70vh` para una lista:
  el marco mide `100%`, y la región se queda con lo que sobra después de la
  cabecera con `flex: 1`. Cada eslabón de esa cadena declara `min-height: 0`,
  porque un hijo de flex no se encoge por debajo de su contenido salvo que se le
  diga, y con la cadena rota la lista crece hasta empujar la página.
- **La fila que reparte usa `align-items: stretch`.** Con `flex-start`, la
  columna mide lo que mide su contenido y el `flex: 1` de dentro no reparte
  nada: con ocho juegos no se nota y con cuatrocientos se desborda. Quien no
  quiera estirarse lo dice por su cuenta con `align-self`.
- **El tope de ancho lo pone la pieza, no el marco.** Un `max-width` en el
  contenedor que se desplaza deja huecos a los lados que no responden a la
  rueda. Se capan la tabla, el formulario o la tarjeta; el hueco que dejan sigue
  siendo parte de la caja que se desplaza, así que la rueda funciona encima de
  él.

## 🏆 Benefits

- **La rueda funciona donde esté el ratón.** Que es lo que uno espera, y lo que
  no pasa cuando el hueco de los lados pertenece a un contenedor que no se
  desplaza.
- **Una barra en pantalla, no dos.** Dos barras obligan a mirar cuál es cuál
  antes de arrastrar, y la de fuera suele tener un recorrido ridículo: se mueve
  un dedo y no pasa nada.
- **La cabecera de la tabla y la barra de lote se quedan donde deben.** Con una
  sola región que se desplaza, `position: sticky` tiene un solo contexto y hace
  lo que promete.
- **El tope de cada pieza se puede justificar por separado.** El de la tabla
  sale de una suma —96rem, más el hueco y el inspector, son los 120rem de una
  pantalla de 1920 maximizada—, y eso hace que abrir una ficha no mueva ni una
  columna.

## 👀 Examples

### ✅ Good

```css
html,
body,
#root {
  height: 100%;
}

main {
  height: 100%;          /* el marco no se desplaza… */
  display: flex;
  flex-direction: column;
}

.library,
.library-body,
.library-main {
  flex: 1;
  min-height: 0;         /* …y la altura baja entera hasta el visor */
}

.tabla-viewport {
  flex: 1;
  min-height: 0;
  overflow: auto;        /* …que es el único que se desplaza */
}

.tabla {
  max-width: 96rem;      /* el tope va aquí, no en `main` */
}
```

### ❌ Bad

```css
main {
  max-width: 80rem;
  margin: 0 auto;
}

.tabla-viewport {
  height: 70vh;
  overflow: auto;
}
```

En una pantalla de 1920 esto deja 320 px muertos a cada lado —la rueda encima de
ellos no encuentra nada que mover— y la suma de la cabecera más el `70vh` más el
relleno se pasa de la ventana por unos cuarenta píxeles: aparece una segunda
barra, la de la página, con cuarenta píxeles de recorrido. Hay que llevar el
ratón al centro para que la lista se mueva.

## 🧐 Real world examples

- [`src/styles.css`](../../src/styles.css) reparte la altura desde `html` hasta
  `.tabla-viewport` y `.pared-viewport`, y pone los topes en `.tabla`,
  `.revision`, `.destacado` y `form`, nunca en `main`.
- [`test/visual/mirar.ts`](../../test/visual/mirar.ts) lo comprueba en las tres
  pantallas y a tres anchos: que ni la página ni el marco se desplacen, que la
  región llegue a los dos bordes y que se quede con un alto que sirva de algo.
  La última condición es la que cazó que la biblioteca se quedaba en 294 px
  mientras las otras dos tenían 644.

## 🔗 Related agreements

- [Una comprobación afirma sobre la estructura, no sobre lo que parece](../testing/afirmar-sobre-la-estructura.md)
  — «solo hay una barra» se mide comparando `scrollHeight` con `clientHeight`,
  no mirando una captura, donde una barra de más pasa desapercibida.
- [Un estado para los dos modos de vista](un-estado-para-los-dos-modos-de-vista.md)
  — la tabla y la pared reparten la altura igual porque cuelgan del mismo sitio.
- [`AGENTS.md`](../../AGENTS.md) — índice de todas las convenciones.
