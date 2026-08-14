//! Todo enlace que ofrezca la interfaz tiene que estar permitido en la
//! capacidad.
//!
//! Este test existe porque el fallo ya se coló dos veces. `opener:allow-open-url`
//! habilita el comando pero **no da alcance a ninguna dirección**, así que
//! durante toda la fase 3 los enlaces del asistente de Steam estuvieron rotos
//! sin que nadie lo notara: en un contenedor sin navegador nadie los pulsa. Y al
//! añadir el alcance se coló un `https://steamid.io/*`, que no casa con
//! `https://steamid.io` porque los patrones se comparan contra la cadena tal
//! cual, sin normalizar y sin barra final.
//!
//! Ninguna de las dos cosas la veía la suite. Ahora sí.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Los patrones de URL permitidos en `capabilities/default.json`.
fn patrones_permitidos() -> Vec<String> {
    let bruto = std::fs::read_to_string(raiz().join("capabilities/default.json"))
        .expect("leer la capacidad");
    let capacidad: serde_json::Value =
        serde_json::from_str(&bruto).expect("la capacidad tiene que ser JSON válido");

    capacidad["permissions"]
        .as_array()
        .expect("permissions es una lista")
        .iter()
        .filter(|permiso| permiso["identifier"] == "opener:allow-open-url")
        .flat_map(|permiso| {
            permiso["allow"]
                .as_array()
                .expect("allow es una lista")
                .iter()
                .filter_map(|entrada| entrada["url"].as_str().map(str::to_owned))
        })
        .collect()
}

/// Las direcciones que la interfaz llega a pasarle a `openUrl`.
///
/// Se leen del propio código en vez de mantener una lista aparte: una lista
/// aparte se queda vieja justo cuando importa, que es al añadir un enlace nuevo.
fn urls_de_la_interfaz() -> BTreeSet<String> {
    let mut urls = BTreeSet::new();
    recorrer(&raiz().join("../src/features"), &mut urls);
    assert!(
        !urls.is_empty(),
        "algo va mal en el test si no encuentra ni un enlace"
    );
    urls
}

fn recorrer(dir: &Path, urls: &mut BTreeSet<String>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.is_dir() {
            recorrer(&ruta, urls);
            continue;
        }
        let es_fuente = ruta
            .extension()
            .is_some_and(|ext| ext == "tsx" || ext == "ts");
        let es_test = ruta
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(".test."));
        if !es_fuente || es_test {
            continue;
        }
        let Ok(codigo) = std::fs::read_to_string(&ruta) else {
            continue;
        };
        urls.extend(extraer_urls(&codigo));
    }
}

/// Saca las cadenas `"https://…"` del código. Basta con esto: en este proyecto
/// las direcciones son constantes literales, nunca se componen a trozos.
fn extraer_urls(codigo: &str) -> Vec<String> {
    codigo
        .match_indices("\"https://")
        .filter_map(|(inicio, _)| {
            let resto = &codigo[inicio + 1..];
            resto.find('"').map(|fin| resto[..fin].to_owned())
        })
        .collect()
}

#[test]
fn todos_los_enlaces_de_la_interfaz_estan_permitidos() {
    let patrones: Vec<glob::Pattern> = patrones_permitidos()
        .iter()
        .map(|p| glob::Pattern::new(p).expect("patrón glob válido"))
        .collect();

    for url in urls_de_la_interfaz() {
        assert!(
            patrones.iter().any(|patron| patron.matches(&url)),
            "la interfaz enlaza a {url} pero ningún patrón de \
             capabilities/default.json lo permite: al pulsarlo saldría \
             «Not allowed to open url»"
        );
    }
}

#[test]
fn no_se_permiten_direcciones_que_ya_nadie_usa() {
    // Un permiso que sobra es alcance regalado. Si un enlace desaparece de la
    // interfaz, su patrón tiene que desaparecer de la capacidad.
    let urls = urls_de_la_interfaz();
    for patron in patrones_permitidos() {
        let compilado = glob::Pattern::new(&patron).expect("patrón glob válido");
        assert!(
            urls.iter().any(|url| compilado.matches(url)),
            "capabilities/default.json permite {patron} y ya no hay ningún \
             enlace en la interfaz que lo necesite"
        );
    }
}
