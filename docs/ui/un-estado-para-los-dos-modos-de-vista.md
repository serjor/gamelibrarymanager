# 🎯 Un estado para los dos modos de vista: las vistas solo pintan

## 💡 Convention

La biblioteca tiene dos presentaciones —tabla y pared de portadas— y **un solo
estado**. El filtro, la ordenación, la selección y qué ficha está abierta viven
en [`Library.tsx`](../../src/features/library/Library.tsx), que aplica filtro y
orden **una vez** y le pasa a la vista de turno el resultado ya calculado.

Una vista recibe lo que tiene que pintar y avisa de lo que el usuario hace. No
filtra, no ordena y no guarda nada que la otra vista también necesite:

```tsx
const compartido = { rows: visible, selected, onSelect, onOpen, abierto };
```

Cuando dos vistas necesitan el mismo gesto, el gesto también sube: la selección
por rango con mayúsculas vive en
[`useSeleccion.ts`](../../src/features/library/useSeleccion.ts) y no dentro de
cada vista.

El corolario está en la otra dirección: **una pantalla que hace sus propios
cortes no es un modo de vista**. «Hoy» no lee los filtros de la biblioteca
porque no comparte contrato con ella; por eso es una pestaña y no un tercer
botón al lado de «Tabla» y «Portadas».

## 🏆 Benefits

- **Cambiar de vista no puede cambiar qué juegos hay delante.** Y eso no es algo
  que haya que comprobar en cada cambio: es que no existen dos sitios donde
  pudiera divergir.
- **Un gesto se comporta igual en las dos.** Si cada vista llevara su cuenta del
  ancla del rango, empezar una selección en la tabla y terminarla en las
  portadas daría dos resultados para el mismo `⇧+clic`.
- **Lo que se estrena solo se escribe una vez.** La barra de edición en lote no
  se entera de qué vista está debajo: le llega la selección y ya está.
- **La vista queda pequeña y se puede tirar.** `LibraryWall` se pudo escribir de
  cero en la fase 4 sin tocar nada de lo que la tabla ya hacía funcionar.

## 👀 Examples

### ✅ Good

```tsx
// Library.tsx: filtrado y orden se aplican aquí, una vez.
const visible = useMemo(
  () => applySort(applyFilters(rows, filters), sort),
  [rows, filters, sort],
);

const compartido = { rows: visible, selected, onSelect: marcar, onOpen, abierto };

return vista === "tabla" ? (
  <LibraryTable {...compartido} sort={sort} onSort={setSort} />
) : (
  <LibraryWall {...compartido} />
);
```

`sort` baja a la tabla porque es la única que enseña cabeceras que ordenan, pero
el estado sigue arriba: la pared pinta ordenado sin saber que existe.

### ❌ Bad

```tsx
// LibraryWall.tsx
export function LibraryWall({ rows, filters }: Props) {
  // Cada vista se filtra lo suyo…
  const visible = useMemo(() => applyFilters(rows, filters), [rows, filters]);
  // …y se guarda su propia selección.
  const [seleccionados, setSeleccionados] = useState<Set<string>>(new Set());
```

Ahora hay dos implementaciones del mismo filtrado, y el día que una gane un caso
—acentos, deseados, lo que sea— la otra se queda atrás sin que falle nada.
Además, marcar cuatro juegos en la tabla y cambiar a portadas los pierde, así
que la barra de lote enseña una cosa distinta según por dónde hayas pasado.

## 🧐 Real world examples

- [`src/features/library/Library.tsx`](../../src/features/library/Library.tsx)
  tiene el estado entero: `filters`, `sort`, `selected` y `abierto`. Ahí está
  también la regla de que cambiar el filtro vacía la selección, para que el lote
  no escriba sobre juegos que ya no se ven.
- [`src/features/library/useSeleccion.ts`](../../src/features/library/useSeleccion.ts)
  guarda el ancla del rango fuera de las dos vistas, con el motivo escrito.
- [`src/features/today/Today.tsx`](../../src/features/today/Today.tsx) es el
  caso contrario, y por eso es una pestaña: calcula sus propios cortes sobre
  `rows` y no recibe `filters`.
- [`src/App.test.tsx`](../../src/App.test.tsx) — «cambiar de vista no cambia qué
  juegos hay delante» y «lo seleccionado en la tabla sigue seleccionado en las
  portadas» son la comprobación de esto.

## 🔗 Related agreements

- [Ningún componente declara un color](tokens-como-unica-fuente-de-color.md) —
  la misma idea aplicada al color: una fuente, muchos consumidores.
- El plan `.agents/plans/0002-rediseno-ui/plan.html` recoge por qué «Hoy» no es
  un modo de vista, con la alternativa descartada.
- [`AGENTS.md`](../../AGENTS.md) — índice de todas las convenciones.
