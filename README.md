# Game Library Manager

App de escritorio que unifica en una sola biblioteca los juegos que tienes
repartidos entre tiendas, con metadatos comunes, sin duplicados y con tu propio
estado de backlog encima.

Local-first: todo vive en un SQLite de tu máquina. No hay servidor, no hay
cuentas y ninguna credencial de tienda sale de tu ordenador.

## Estado

Fases 1 a 7: Steam, GOG, Epic, metadatos de IGDB, deduplicación entre tiendas y
backlog. Queda especificada y sin implementar la fase 8 (precios con ITAD).

Puedes empezar solo con Steam, que es la única tienda con una vía oficial: hace
falta tu clave de la API. GOG y Epic se conectan en su propia página de login,
dentro de la app; tu contraseña no pasa por aquí. Como ninguna de las dos
permite registrar aplicaciones de terceros, hay que darles además el par de
cliente de su propio lanzador, que es público y el mismo para todo el mundo: se
te pide para que no vaya escrito dentro del programa.

Epic es la tienda sin ningún contrato público: se apoya en la API privada de su
lanzador y puede dejar de funcionar el día que a Epic le parezca. Por eso cada
conector tiene su propio interruptor. Si Epic se rompe, lo desactivas, y lo que
ya trajo sigue en tu biblioteca con tus notas y tu estado encima.

Los metadatos de IGDB son opcionales. Sin ellos la biblioteca funciona igual,
con las fichas hechas a partir del título de la tienda —incluida la
deduplicación entre tiendas—, y el día que configures IGDB esas fichas se
enriquecen en su sitio sin perder el estado que hayas escrito encima.

Ninguna de esas credenciales sale de tu ordenador: viven en el keyring del
sistema, o en un fichero cifrado si tu escritorio no tiene uno.

El plan completo, con las decisiones tomadas y sus alternativas descartadas,
está en
[`.agents/plans/0001-game-library-manager/plan.html`](.agents/plans/0001-game-library-manager/plan.html).

## Desarrollo

Requiere Rust estable, [Bun](https://bun.sh) y, en Linux, las dependencias de
sistema de Tauri (`libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `librsvg2-dev`,
`libayatana-appindicator3-dev`).

```sh
bun install
bun run tauri dev
```

Si en Wayland la ventana muere al arrancar con `Error 71 (Protocol error)
dispatching to Wayland display`, es el renderizador dmabuf de WebKitGTK, no la
app:

```sh
WEBKIT_DISABLE_DMABUF_RENDERER=1 bun run tauri dev
```

Comprobaciones, las mismas que ejecuta CI:

```sh
cargo fmt --check && cargo clippy --all-targets --workspace -- -D warnings && cargo test --workspace
bunx tsc --noEmit && bun run lint && bun test
```

## Arquitectura

| Crate | Responsabilidad |
| --- | --- |
| `crates/domain` | Entidades y reglas. Sin red, sin base de datos, sin Tauri. CI lo verifica. |
| `crates/storage` | SQLite y migraciones. Todo el SQL del proyecto está aquí. |
| `crates/connectors` | Tiendas (Steam, GOG, Epic), solo autenticación y listado. Nunca descargas. |
| `crates/metadata` | Proveedores de metadatos (IGDB). |
| `crates/secrets` | Keyring nativo del sistema operativo. |
| `src-tauri` | Shell de la app y comandos: orquestan, no deciden. |
| `src` | UI en React, organizada por feature. |

### Por qué es una app de escritorio y no una web

Steam es la única de las tres tiendas con una vía oficial para leer tu
biblioteca. GOG y Epic no tienen API pública, y su autenticación solo es
defendible ejecutándose en tu máquina: un servidor que guardase esos tokens
incumpliría sus términos de uso, sería un objetivo de ataque y un solo bloqueo
de IP dejaría sin servicio a todos los usuarios a la vez. Por eso Playnite,
Heroic y Lutris son aplicaciones de escritorio, y por eso esta también.

## Licencia

GPL-3.0-or-later. Ver [LICENSE](LICENSE) y [NOTICE](NOTICE).
