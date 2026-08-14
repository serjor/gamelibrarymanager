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
//!
//! El segundo test no comprueba que no sobren patrones, aunque sería lo
//! simétrico: hay direcciones —la ficha de un juego en su tienda— que se
//! construyen con datos y no aparecen literales en ninguna parte, y una de
//! ellas la manda GOG en su propia respuesta. Lo que sí se puede exigir, y es
//! lo que de verdad protege, es que ningún patrón abra un host entero.

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

/// Las direcciones constantes que la interfaz llega a pasarle a `openUrl`.
///
/// Se leen del propio código en vez de mantener una lista aparte: una lista
/// aparte se queda vieja justo cuando importa, que es al añadir un enlace nuevo.
///
/// Solo se mira `src/`. Rastrear también los conectores sería tentador, pero
/// sus literales son sobre todo endpoints que el programa **llama**, no páginas
/// que el usuario **abre**, y desde fuera no se distinguen: `https://api.gog.com`
/// no debe estar permitida y aparecería igual. Las direcciones que sí se abren y
/// no son constantes se cubren en el test de más abajo, con ejemplos.
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
fn las_paginas_de_una_copia_y_de_una_ficha_estan_permitidas() {
    // Estas no son constantes: la de Steam la construye el conector con el
    // appid, la de GOG la manda la propia tienda en su respuesta y la de IGDB
    // sale del slug del candidato. No hay literal que rastrear, así que se
    // comprueban con ejemplos reales —los mismos que salen en las fixtures—.
    let ejemplos = [
        "https://store.steampowered.com/app/292030",
        "https://www.gog.com/game/the_witcher_2",
        "https://www.igdb.com/games/the-witcher-3-wild-hunt",
    ];

    let patrones: Vec<glob::Pattern> = patrones_permitidos()
        .iter()
        .map(|p| glob::Pattern::new(p).expect("patrón glob válido"))
        .collect();

    for url in ejemplos {
        assert!(
            patrones.iter().any(|patron| patron.matches(url)),
            "la cola de revisión abre direcciones como {url} y ningún patrón de \
             capabilities/default.json lo permite"
        );
    }
}

#[test]
fn ningun_patron_abre_un_host_entero() {
    // Un permiso que sobra es alcance regalado, y la forma de regalarlo de
    // verdad es un comodín en el host: `https://*` es `allow-default-urls` con
    // otro nombre, y `https://*.algo.com` abre cualquier subdominio que alguien
    // registre. En la ruta el comodín sí hace falta, porque hay direcciones que
    // se construyen con el identificador de cada juego.
    for patron in patrones_permitidos() {
        let host = patron
            .strip_prefix("https://")
            .unwrap_or_else(|| panic!("{patron} tiene que ser https"))
            .split('/')
            .next()
            .unwrap_or_default();

        assert!(
            !host.contains('*') && !host.contains('?'),
            "capabilities/default.json permite {patron}: el host no puede llevar \
             comodín, o el alcance deja de acotar nada"
        );
    }
}
