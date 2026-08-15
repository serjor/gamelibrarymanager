# AGENTS.md

Índice de las convenciones del proyecto. Cada una vive en su propio fichero
dentro de `docs/`, organizado por área del repositorio.

Antes de escribir código, lee también:

- [`README.md`](README.md) — arquitectura, crates y sus fronteras.
- `.agents/plans/0001-game-library-manager/plan.html` — el plan acordado, con las
  decisiones cerradas **y sus alternativas descartadas**. No se relitigan: si
  crees que alguna está mal, dilo y espera respuesta en vez de cambiarla.
- [`docs/documentation-guidelines.md`](docs/documentation-guidelines.md) — cómo
  se escribe y dónde va un documento nuevo.

## Convenciones

### `docs/connectors/` — tiendas

| Convención | De qué trata |
| --- | --- |
| [Ninguna credencial de tienda va dentro del binario](docs/connectors/credenciales-fuera-del-binario.md) | Todo lo aporta el usuario y vive en el almacén de secretos, incluso cuando el secreto es público. Nunca se pide la contraseña de una tienda. |
| [Los endpoints no oficiales se contrastan antes de escribir el conector](docs/connectors/contrastar-endpoints-no-oficiales.md) | Leer la implementación de referencia viva, probar a mano, y anotar la vigencia con fecha en el módulo. |
| [Every store connector has a switch of its own](docs/connectors/switch-per-connector.md) | Una tienda rota se apaga y el resto sigue igual. El motivo se guarda, apagar es decisión del usuario y nada más la toma. |

### `docs/storage/` — esquema y datos

| Convención | De qué trata |
| --- | --- |
| [Enriquecer una ficha reescribe su fila; no crea otra](docs/storage/enriquecer-fichas-en-su-sitio.md) | `user_state` cuelga del `game_id`: reutilizarlo es lo que impide perder lo que el usuario escribió. |

### `docs/tauri/` — shell de la aplicación

| Convención | De qué trata |
| --- | --- |
| [Todo enlace de la interfaz necesita alcance explícito en la capacidad](docs/tauri/alcance-de-urls-en-capacidades.md) | `opener:allow-open-url` habilita el comando pero no da alcance, y los patrones se comparan sin normalizar. |
| [A script in a store login window runs on one page and carries no logic](docs/tauri/scripts-in-a-login-window.md) | Leer la página de una tienda solo se hace donde emite el código, sin lógica dentro del script y sin darle comandos. |

### Pendientes de documentar

Áreas previstas y todavía vacías: `docs/domain/`, `docs/ui/`, `docs/testing/`.

## Comprobaciones

Las mismas que ejecuta CI. Todas tienen que pasar antes de dar una fase por
cerrada:

```sh
cargo fmt --all --check
cargo clippy --all-targets --workspace -- -D warnings
cargo test --workspace
bunx tsc --noEmit && bun run lint && bun test
bun run tauri dev            # tiene que abrir la ventana
```

Hay una comprobación que CI **no** puede hacer, porque necesita una sesión de
escritorio con secret-service:

```sh
cargo test -p secrets --test keyring_real -- --ignored
```
