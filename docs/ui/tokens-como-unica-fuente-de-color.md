# 🎯 Ningún componente declara un color: todos salen de los tokens

## 💡 Convention

Todos los colores de la interfaz se definen **una vez**, como variables CSS en
el `:root` de [`src/styles.css`](../../src/styles.css), y su variante oscura se
define **en el mismo sitio**, dentro del único bloque
`@media (prefers-color-scheme: dark)`. A partir de ahí, ninguna regla y ningún
componente escribe un color: escribe `var(--token)`.

Un color que no sea un token no existe. Eso incluye los que parecen inofensivos:

- Los literales sueltos (`#b3261e`, `rgb(0 0 0 / .5)`).
- Las palabras clave del sistema (`Canvas`, `ButtonFace`), que cambian de valor
  según el tema del escritorio y no se pueden comparar con nuestros tokens.
- Los colores derivados de `currentColor`, que valen una cosa u otra según quién
  sea el padre.

Lo que sí se puede es **derivar** un token de otro con `color-mix`, siempre que
el resultado se declare también como token o se use en una sola regla:

```css
--velo: color-mix(in srgb, var(--texto) 45%, transparent);
```

Y hay un caso que obliga a definir el mismo token dos veces con tokens de origen
distintos: cuando lo que hace falta en los dos temas es lo mismo —oscurecer—
pero el token que es oscuro no es el mismo en cada uno.

La regla se puede comprobar de un vistazo:

```sh
grep -nE '#[0-9a-fA-F]{3,6}' src/styles.css
```

Solo debe devolver líneas dentro del bloque donde se definen los tokens.

## 🏆 Benefits

- **El tema oscuro se toca en un sitio.** Un `#b3261e` a mitad de la hoja obliga
  a buscarlo a mano el día que haya que ajustar el oscuro, y garantiza que
  alguno se quede sin ajustar. Con tokens, el bloque oscuro es la lista completa
  de lo que hay que decidir.
- **El contraste se puede medir.** Si el color del texto y el de su fondo salen
  de tokens, una comprobación puede leer los dos y calcular la razón. Con
  colores repartidos, cada sitio es un caso nuevo.
- **Los nombres dicen para qué es cada color, no cómo se ve.** `--estado-jugando`
  sobrevive a cambiar el azul; `--azul-claro` no.
- **Separar el acento de los semánticos evita que la interfaz grite.** El acento
  marca dónde estás; los `--estado-*` dicen qué es cada juego. Cuando se
  confunden, la pantalla enseña cuatro cosas urgentes a la vez.

## 👀 Examples

### ✅ Good

```css
:root {
  --error: #b3261e;
  --estado-jugando: #1f6b86;
}

@media (prefers-color-scheme: dark) {
  :root {
    --error: #f2857b;
    --estado-jugando: #79c0dd;
  }
}

[role="alert"] {
  color: var(--error);
}

.estado.playing {
  color: var(--estado-jugando);
}
```

El componente no sabe de qué color es un error. Sabe que es un error.

### ❌ Bad

```css
[role="alert"] {
  color: #b3261e;
}

@media (prefers-color-scheme: dark) {
  [role="alert"] {
    color: #f2857b;
  }
}

/* Y en otro sitio de la hoja, la barra que tapa lo que pasa por debajo: */
.sticky {
  background: Canvas;
}
```

Son tres problemas en once líneas. El color del error vive en dos reglas que hay
que acordarse de tocar juntas; el bloque oscuro deja de ser la lista de lo que
hay que decidir y pasa a estar repartido por toda la hoja; y `Canvas` no es el
fondo de la aplicación sino el que decida el escritorio, así que la barra tapa
con un color que no es el de la página.

## 🧐 Real world examples

- [`src/styles.css`](../../src/styles.css) define la paleta entera en `:root` y
  su variante oscura en un único bloque, incluidos los semánticos de estado de
  partida y el velo de la hoja de la ficha.
- [`test/visual/mirar.ts`](../../test/visual/mirar.ts) mide el contraste real
  del texto y del texto atenuado contra su fondo, en claro y en oscuro, y contra
  el fondo de la hoja, que es la única superficie que no se pinta sobre el de la
  página. Puede hacerlo justamente porque los dos colores salen de tokens.
- El plan `.agents/plans/0002-rediseno-ui/plan.html` cierra la fase 1 con esta
  regla como criterio: la aplicación tenía que seguir viéndose exactamente igual
  después de aplicarla.

## 🔗 Related agreements

- [Una comprobación afirma sobre la estructura, no sobre lo que parece](../testing/afirmar-sobre-la-estructura.md)
  — por qué el contraste se calcula y no se mira en una captura.
- [Un estado para los dos modos de vista](un-estado-para-los-dos-modos-de-vista.md)
  — la misma idea aplicada al estado en vez de al color: una fuente, muchos
  consumidores.
- [`AGENTS.md`](../../AGENTS.md) — índice de todas las convenciones.
