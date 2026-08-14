# Game Library Manager

App de escritorio que unifica en una sola biblioteca los juegos que tienes
repartidos entre tiendas, con metadatos comunes, sin duplicados y con tu propio
estado de backlog encima.

Local-first: todo vive en un SQLite de tu máquina. No hay servidor, no hay
cuentas y ninguna credencial de tienda sale de tu ordenador.

## Estado

Fase 1 de 8 (andamiaje). El plan completo, con las decisiones tomadas y sus
alternativas descartadas, está en
[`.agents/plans/0001-game-library-manager/plan.html`](.agents/plans/0001-game-library-manager/plan.html).

## Desarrollo

Requiere Rust estable, [Bun](https://bun.sh) y, en Linux, las dependencias de
sistema de Tauri (`libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `librsvg2-dev`,
`libayatana-appindicator3-dev`).

```sh
bun install
bun run tauri dev
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
| `crates/connectors` | Tiendas, solo autenticación y listado. Nunca descargas. |
| `crates/metadata` | Proveedores de metadatos (IGDB). |
| `crates/secrets` | Keyring nativo del sistema operativo. |
| `src-tauri` | Shell de la app y comandos: orquestan, no deciden. |
| `src` | UI en React, organizada por feature. |

## Licencia

GPL-3.0-or-later. Ver [LICENSE](LICENSE) y [NOTICE](NOTICE).
