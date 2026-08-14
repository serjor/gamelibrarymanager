# 🎯 Cómo se escribe la documentación de este repositorio

## 💡 Convention

Cada convención vive en **su propio fichero** dentro de `docs/<area>/`, y las
áreas son las del propio repositorio, no categorías genéricas:

```
docs/
  connectors/  tiendas: endpoints, credenciales, autenticación
  domain/      reglas puras: emparejamiento, normalización
  storage/     esquema, capas, migraciones
  tauri/       capacidades, comandos, ventanas
  ui/          React, onboarding
  testing/     fixtures, tests guardianes
```

Todo documento lleva estas secciones, en este orden y con estos emojis:

```markdown
# 🎯 Nombre de la convención

## 💡 Convention
## 🏆 Benefits
## 👀 Examples   (con subsecciones ✅ Good y ❌ Bad)
## 🧐 Real world examples
## 🔗 Related agreements
```

Reglas que no se negocian:

- **Una convención por fichero.** Si un documento explica dos cosas, son dos
  documentos.
- **Los ejemplos son código de verdad**, con su ✅ y su ❌. El ❌ es el que
  enseña: sin él, la convención parece una preferencia.
- **«Real world examples» apunta a ficheros de este repositorio**, con ruta y
  línea. Un ejemplo inventado envejece sin que nadie se entere; uno que apunta
  al código se rompe a la vista.
- **Se explica el porqué, no el qué.** El qué ya está en el código.
- **En español**, como el resto del proyecto.
- Toda alta se añade al índice de [`AGENTS.md`](../AGENTS.md).

## 🏆 Benefits

- Una convención por fichero se puede enlazar, discutir y borrar por separado.
  Un documento cajón de sastre no se actualiza nunca porque tocarlo da miedo.
- Las áreas por crate hacen obvio dónde buscar y dónde escribir: son las mismas
  fronteras que la arquitectura ya obliga a respetar.
- La estructura fija hace que leer el quinto documento cueste lo mismo que leer
  el primero.
- Enlazar a ficheros reales convierte la documentación en algo comprobable en
  vez de en una promesa.

## 👀 Examples

### ✅ Good

Escrito desde `docs/tauri/`, de ahí el `../../` hasta la raíz del repositorio:

```markdown
## 🧐 Real world examples

- [`src-tauri/capabilities/default.json`](../../src-tauri/capabilities/default.json)
  enumera las cuatro direcciones del asistente.
- [`src-tauri/tests/capacidades.rs`](../../src-tauri/tests/capacidades.rs)
  falla si una de ellas deja de estar permitida.
```

### ❌ Bad

```markdown
## 🧐 Real world examples

- El fichero de capacidades tiene la lista de URLs permitidas.
- Hay un test que lo comprueba.
```

Sin ruta no se puede ir a mirar, y cuando el fichero se mueva nadie se dará
cuenta de que el documento ya miente.

## 🔗 Related agreements

- [`AGENTS.md`](../AGENTS.md) — índice de todas las convenciones.
- [`README.md`](../README.md) — arquitectura y crates, que es de donde salen
  las áreas.
- El plan en `.agents/plans/0001-game-library-manager/plan.html` recoge las
  decisiones cerradas con sus alternativas descartadas. La documentación
  desarrolla esas decisiones; no las relitiga.
