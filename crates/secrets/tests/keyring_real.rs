//! Comprobación del keyring **real** del sistema.
//!
//! Está marcada `#[ignore]` a propósito: CI y los contenedores no tienen
//! secret-service, y ahí el resultado correcto es justamente el contrario del
//! que se afirma aquí. Se ejecuta a mano en una sesión de escritorio:
//!
//! ```sh
//! cargo test -p secrets --test keyring_real -- --ignored --nocapture
//! ```
//!
//! Existe porque el camino del keyring nativo se escribió en un contenedor
//! headless, donde `detect` siempre elegía el fichero cifrado: hasta la primera
//! ejecución en un escritorio de verdad, nadie había comprobado la otra rama.

use secrets::{Backend, KeyringStore, SecretStore};

/// Servicio propio para no ensuciar el del usuario si la prueba se corta a
/// mitad.
const SERVICE: &str = "com.serjor.gamelibrarymanager.test";

#[test]
#[ignore = "necesita una sesión de escritorio con secret-service"]
fn detecta_el_keyring_y_guarda_y_lee() {
    assert_eq!(
        secrets::detect(SERVICE),
        Backend::Keyring,
        "en un escritorio con secret-service la detección debe elegir el keyring"
    );

    let store = KeyringStore::new(SERVICE);
    let key = "prueba:credencial";
    let value = "valor-de-ida-y-vuelta";

    store.set(key, value).expect("guardar en el keyring");
    assert_eq!(
        store.get(key).expect("leer del keyring").as_deref(),
        Some(value),
        "lo leído tiene que ser exactamente lo guardado"
    );

    store.delete(key).expect("borrar del keyring");
    assert_eq!(
        store.get(key).expect("leer tras borrar"),
        None,
        "borrar tiene que dejar la entrada sin valor, no fallar"
    );

    // Borrar algo que no existe es correcto y no un error: la sincronización
    // llama a `delete` sin saber si había credencial previa.
    store.delete(key).expect("borrar dos veces es idempotente");
}
