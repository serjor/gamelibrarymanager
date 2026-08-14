# 🎯 Todo enlace de la interfaz necesita alcance explícito en la capacidad

## 💡 Convention

Para que la interfaz pueda abrir una dirección en el navegador hacen falta **dos
cosas**, no una:

1. El **comando**: `opener:allow-open-url`.
2. El **alcance**: una lista `allow` con patrones que casen con esa dirección.

`opener:allow-open-url` por sí solo habilita el comando *«without any
pre-configured scope»*, así que sin la lista **todas** las direcciones se
rechazan con `Not allowed to open url`.

Y los patrones se comparan con `glob::Pattern` contra **la cadena tal cual la
pasa la interfaz**: el plugin no normaliza la URL, no le añade la barra final ni
la reescribe de ninguna forma. Un patrón tiene que casar carácter a carácter.

Por eso:

- Se enumeran las direcciones concretas, **no** se usa
  `opener:allow-default-urls`, que abre de par en par todo `http://` y
  `https://`.
- Cada patrón se escribe igual que la constante de la interfaz que lo va a usar.
- El comodín vale en la **ruta**, nunca en el **host**: `https://www.gog.com/game/*`
  acota, `https://*.gog.com` no acota nada.

Hay dos clases de dirección y se comprueban distinto, porque no se pueden
comprobar igual:

| Origen | Ejemplo | Cómo se comprueba |
| --- | --- | --- |
| Constante de la interfaz | la página de la clave de Steam | El test la rastrea en `src/` y exige que algún patrón la permita |
| Construida con datos | la ficha de un juego en su tienda | No hay literal que rastrear: el test usa ejemplos reales |

El test [`capacidades`](../../src-tauri/tests/capacidades.rs) **no** comprueba
que no sobren patrones, aunque sería lo simétrico. Rastrear los conectores para
averiguarlo no sirve: sus literales son sobre todo endpoints que el programa
*llama* —`https://api.gog.com`— y no páginas que el usuario *abre*, y desde
fuera son indistinguibles. Permitir uno por error sería peor que el problema que
se quería resolver. Lo que sí se exige, y es lo que de verdad protege del
alcance regalado, es que ningún patrón lleve comodín en el host.

Además, el `catch` de un `openUrl` **incluye la causa**. Tragársela convierte un
permiso mal puesto en un misterio.

## 🏆 Benefits

- El asistente de onboarding funciona. Sin alcance, los enlaces de «Sacar mi
  clave» y «Averiguar mi SteamID» estuvieron rotos toda la fase 3 sin que nadie
  lo notara: en un contenedor sin navegador nadie los pulsa.
- El alcance mínimo es alcance real. Con `allow-default-urls`, cualquier código
  que acabe en el webview puede abrir cualquier página; con cuatro direcciones
  enumeradas, no.
- El test cierra una clase entera de fallo que ninguna otra comprobación del
  proyecto veía: ni `tsc`, ni `eslint`, ni `clippy`, ni la suite de Rust miran
  un JSON de capacidades contra unas constantes de TypeScript.
- Un error con su causa dentro se diagnostica leyéndolo. Sin ella hay que ir a
  buscar el permiso a mano.

## 👀 Examples

### ✅ Good

```json
{
  "identifier": "opener:allow-open-url",
  "comment": "Los patrones se comparan con la cadena tal cual la pasa la interfaz, sin normalizar, así que tienen que coincidir carácter a carácter con las constantes de src/features/onboarding/.",
  "allow": [
    { "url": "https://steamcommunity.com/dev/apikey" },
    { "url": "https://steamid.io" },
    { "url": "https://www.igdb.com/games/*" },
    { "url": "https://store.steampowered.com/app/*" }
  ]
}
```

Y la constante se escribe entera, para que el test pueda encontrarla:

```tsx
// Concatenar una constante deja `https://www.igdb.com/games/` en el código,
// que es lo que el test rastrea.
const IGDB_GAME_URL = "https://www.igdb.com/games/";
abrir(IGDB_GAME_URL + candidate.slug);
```

```tsx
// La causa viaja con el mensaje: es lo que convierte «no se abre» en
// «Not allowed to open url», que ya dice dónde mirar.
openUrl(url).catch((cause: unknown) =>
  setError(`No he podido abrir ${url}: ${errorMessage(cause)}`),
);
```

### ❌ Bad

```json
{
  "permissions": ["core:default", "opener:allow-open-url"]
}
```

Habilita el comando y no permite ni una dirección: **todos** los enlaces fallan.

```json
{ "url": "https://steamid.io/*" }
```

No casa con `https://steamid.io`. El patrón exige una `/` literal detrás del
host y la constante de la interfaz no la lleva; el plugin no la añade.

```json
{ "url": "https://*" }
```

Equivale a `allow-default-urls`: alcance regalado a cambio de ahorrarse cuatro
líneas.

```tsx
// Interpolar la dirección entera no deja ninguna constante que rastrear: el
// test no puede comprobarla y el fallo vuelve a ser invisible.
abrir(`https://www.igdb.com/games/${candidate.slug}`);
```

```tsx
// Se come el motivo y deja al usuario —y a quien lo depure— sin nada.
openUrl(url).catch(() => setError(`No he podido abrir ${url}`));
```

## 🧐 Real world examples

- [`src-tauri/capabilities/default.json`](../../src-tauri/capabilities/default.json)
  — las cuatro direcciones del asistente, con el porqué en el campo `comment`.
- [`src-tauri/tests/capacidades.rs`](../../src-tauri/tests/capacidades.rs) — saca
  las constantes `https://` de `src/features/` y comprueba que cada una está
  permitida. Las lee del código en vez de mantener una lista aparte, que es lo
  que se queda viejo justo cuando importa. Las direcciones construidas con datos
  van con ejemplos, y un tercer test prohíbe el comodín en el host.
- [`src/features/review/ReviewQueue.tsx`](../../src/features/review/ReviewQueue.tsx)
  — `IGDB_GAME_URL` es la constante que hace rastreable un enlace que se arma
  con el slug de cada candidato.
- [`src/features/onboarding/SteamSetup.tsx`](../../src/features/onboarding/SteamSetup.tsx),
  [`IgdbSetup.tsx`](../../src/features/onboarding/IgdbSetup.tsx) y
  [`GogSetup.tsx`](../../src/features/onboarding/GogSetup.tsx) — las constantes
  que el test lee, y el `catch` que propaga la causa.

## 🔗 Related agreements

- [Contrastar los endpoints no oficiales antes de escribir el conector](../connectors/contrastar-endpoints-no-oficiales.md)
  — la misma idea aplicada fuera: comprobar en vez de suponer.
- [Las credenciales de tienda no van en el binario](../connectors/credenciales-fuera-del-binario.md)
  — la otra mitad del capítulo de permisos.
