# 🎯 Ninguna credencial de tienda va dentro del binario

## 💡 Convention

Ni una clave, ni un secreto, ni un token empotrado en el binario o en el
repositorio. **Todas** las credenciales las aporta el usuario al conectar la
cuenta y viven en el almacén de secretos
([`crates/secrets`](../../crates/secrets/src/lib.rs)): keyring nativo del sistema
operativo, o fichero cifrado con contraseña si la sesión no tiene secret-service.

Nunca en SQLite, nunca en un fichero de configuración, nunca en un log.

La regla aguanta incluso cuando el secreto es público. GOG no permite registrar
aplicaciones de terceros: el único cliente que su servidor de autorización
reconoce es el de GOG Galaxy, y su par `client_id`/`client_secret` lleva años
publicado en gogdl. Aun así **no se empotra**: se le pide al usuario, va al
almacén junto a los tokens, y la pantalla dice sin adornos que no es una clave
suya y que es la misma para todo el mundo. Llamarla «tu clave» sería mentir; que
viaje dentro del binario sería saltarse la regla por comodidad.

Tampoco se pide **nunca** usuario y contraseña de una tienda. Steam va por clave
de API propia; GOG, por su propia página de login en un webview del que solo se
captura el `code` de la redirección.

Consecuencia práctica: la credencial que el conector emite es un bloque **opaco**
(`StoreSession::credential`). El resto del sistema lo mueve entre el almacén y el
conector sin interpretarlo, y lleva dentro lo que ese conector necesite para
renovarse solo —incluidas las credenciales de cliente—.

## 🏆 Benefits

- Un binario sin secretos no filtra nada al distribuirse, y el repositorio se
  puede leer entero sin encontrar una clave.
- El acuerdo de desarrollador de IGDB **prohíbe** empotrar el client secret en
  aplicaciones de escritorio. Cumplirlo con una regla general sale más barato que
  con una excepción por proveedor.
- La clave propia de Steam es además lo que desbloquea la biblioteca privada sin
  abrir el perfil: pedirla no es solo una obligación, es lo que hace viable el
  producto.
- Sin credenciales compartidas no hay cuota compartida agotable ni un único
  bloqueo de IP que tumbe a todos los usuarios a la vez.
- Guardar las credenciales de cliente **dentro** de la credencial opaca hace que
  el refresco funcione solo, sin volver a molestar al usuario en cada caducidad.

## 👀 Examples

### ✅ Good

```rust
/// Credenciales de *cliente* de una tienda: identifican a la aplicación, no al
/// usuario. Las aporta el usuario al conectar la cuenta, igual que la clave de
/// Steam. GOG no permite registrar un cliente propio, así que la única forma de
/// no llevar un secreto dentro del binario es que el par entre por la misma
/// puerta que las demás claves y viva en el almacén.
pub struct ClientCredentials {
    pub client_id: String,
    pub client_secret: String,
}
```

```rust
// La credencial es opaca y lleva dentro lo que hace falta para renovarse.
struct GogCredential {
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    user_id: String,
    expires_at: i64,
}
```

```rust
// La credencial va al almacén. La base de datos solo sabe que la cuenta existe,
// no cómo entrar en ella.
state
    .secrets()
    .await?
    .set(&credential_key(&account), &session.credential)?;
```

### ❌ Bad

```rust
// Empotrado en el binario: publicado en cuanto alguien mire el ejecutable, y
// contra el acuerdo de desarrollador en el caso de IGDB.
const GOG_CLIENT_SECRET: &str = "9d85c43b1482497dbbce61f6e4aa173a4337…";
```

```rust
// En SQLite: sin cifrar, y sobrevive a cualquier copia de seguridad del fichero.
sqlx::query("UPDATE store_account SET api_key = ? WHERE id = ?")
```

```tsx
// Pedir la contraseña de la tienda. Los términos de uso de Steam lo prohíben
// expresamente, y para GOG y Epic no hace ninguna falta.
<label>Contraseña de GOG</label>
```

```tsx
// Mentir sobre lo que se está pidiendo. El par de Galaxy no es del usuario y
// es idéntico para todo el mundo: decirlo es parte de la convención.
<p>Introduce tu clave privada de GOG</p>
```

## 🧐 Real world examples

- [`crates/domain/src/ports.rs`](../../crates/domain/src/ports.rs) — `AuthContext`
  y `ClientCredentials`: el contrato que obliga a que el par entre desde fuera.
- [`crates/connectors/src/gog/mod.rs`](../../crates/connectors/src/gog/mod.rs) —
  `GogCredential` y el canje del código; nada de esto se guarda en el binario.
- [`src-tauri/src/commands/gog.rs`](../../src-tauri/src/commands/gog.rs) — abre la
  página real de GOG y solo ve el `code`; la contraseña no pasa por el proceso.
- [`src/features/onboarding/GogSetup.tsx`](../../src/features/onboarding/GogSetup.tsx)
  — la pantalla que dice explícitamente que el par no es del usuario.
- [`crates/secrets/src/lib.rs`](../../crates/secrets/src/lib.rs) — `SecretStore` y
  `detect`, que comprueba de verdad si el keyring responde en vez de suponerlo
  por la plataforma.
- [`src-tauri/tests/sync.rs`](../../src-tauri/tests/sync.rs) — comprueba leyendo
  los bytes del SQLite que la clave de API no aparece en la base de datos.
- [`NOTICE`](../../NOTICE) — registra de dónde sale el par de Galaxy y deja claro
  que el programa no lo lleva dentro.

## 🔗 Related agreements

- [Contrastar los endpoints no oficiales antes de escribir el conector](contrastar-endpoints-no-oficiales.md)
  — la otra mitad de lo que hace falta para añadir una tienda.
- [Todo enlace de la interfaz necesita alcance explícito en la capacidad](../tauri/alcance-de-urls-en-capacidades.md)
  — mínimo privilegio aplicado a los permisos de la ventana.
