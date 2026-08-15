//! A test of the **real** keyring of the system.
//!
//! It is marked `#[ignore]` deliberately: CI and the containers have no
//! secret-service, and there the correct result is exactly the opposite of what
//! this file declares. You run it by hand in a desktop session:
//!
//! ```sh
//! cargo test -p secrets --test keyring_real -- --ignored --nocapture
//! ```
//!
//! It exists because the path of the native keyring was written in a headless
//! container, where `detect` always selected the encrypted file: until the first
//! run on a real desktop, nobody had examined the other branch.

use secrets::{Backend, KeyringStore, SecretStore};

/// A service of its own, so that the service of the user does not become dirty
/// if the test stops in the middle.
const SERVICE: &str = "com.serjor.gamelibrarymanager.test";

#[test]
#[ignore = "it needs a desktop session with secret-service"]
fn it_detects_the_keyring_and_keeps_and_reads() {
    assert_eq!(
        secrets::detect(SERVICE),
        Backend::Keyring,
        "on a desktop with secret-service the detection must select the keyring"
    );

    let store = KeyringStore::new(SERVICE);
    let key = "prueba:credencial";
    let value = "a-value-that-goes-and-comes-back";

    store.set(key, value).expect("guardar en el keyring");
    assert_eq!(
        store.get(key).expect("leer del keyring").as_deref(),
        Some(value),
        "what is read must be exactly what was saved"
    );

    store.delete(key).expect("borrar del keyring");
    assert_eq!(
        store.get(key).expect("leer tras borrar"),
        None,
        "a delete must leave the entry with no value, not fail"
    );

    // To delete something that does not exist is correct and not an error: the
    // synchronisation calls `delete` without it knows whether there was an
    // earlier credential.
    store.delete(key).expect("a second delete is idempotent");
}
