//! El "done when" de la fase 4, sobre 52 títulos reales tal y como los escriben
//! las tiendas.
//!
//! El criterio no es simétrico y eso es deliberado: **cero falsos positivos**.
//! Un juego que se queda sin emparejar aparece en la cola de revisión y el
//! usuario lo arregla en dos clics; dos juegos distintos fusionados le hacen
//! perder el estado y las notas de uno de los dos, y encima sin avisar.

use domain::matching::{Candidate, MatchDecision, decide_by_title, normalize};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    title: String,
    year: Option<i32>,
    /// Un `igdb_id` esperado, o la cadena "review".
    expected: serde_json::Value,
    candidates: Vec<Candidate>,
}

fn corpus() -> Vec<Case> {
    serde_json::from_str(include_str!("fixtures/corpus.json")).expect("corpus ilegible")
}

#[test]
fn ningun_falso_positivo_y_precision_suficiente() {
    let cases = corpus();
    let mut aciertos = 0usize;
    let mut falsos_positivos = Vec::new();
    let mut sin_emparejar = Vec::new();

    for case in &cases {
        let decision = decide_by_title(&case.title, case.year, &case.candidates);
        let esperado = case.expected.as_i64();

        match (&decision, esperado) {
            (MatchDecision::Auto { igdb_id, .. }, Some(quiero)) if *igdb_id == quiero => {
                aciertos += 1;
            }
            // Enlazó algo distinto de lo esperado, o enlazó donde había que
            // preguntar. Este es el error que no se tolera.
            (MatchDecision::Auto { igdb_id, .. }, _) => {
                falsos_positivos.push(format!(
                    "«{}» se enlazó con {} y debía ser {:?}",
                    case.title, igdb_id, case.expected
                ));
            }
            (MatchDecision::Review { .. }, None) => aciertos += 1,
            (MatchDecision::Review { .. }, Some(_)) => sin_emparejar.push(case.title.clone()),
        }
    }

    assert!(
        falsos_positivos.is_empty(),
        "falsos positivos, que es el único error inaceptable:\n{}",
        falsos_positivos.join("\n")
    );

    let precision = aciertos as f64 / cases.len() as f64;
    println!(
        "corpus: {aciertos}/{} correctos ({:.1}%), {} sin emparejar, 0 falsos positivos",
        cases.len(),
        precision * 100.0,
        sin_emparejar.len()
    );
    assert!(
        precision >= 0.95,
        "precisión {:.1}% sobre {} casos (mínimo 95%). Sin emparejar:\n{}",
        precision * 100.0,
        cases.len(),
        sin_emparejar.join("\n")
    );
}

#[test]
fn los_candidatos_de_la_cola_llegan_ordenados_y_acotados() {
    let candidates: Vec<Candidate> = (1..=10)
        .map(|i| Candidate {
            igdb_id: i,
            name: format!("Juego {i}"),
            alternative_names: vec![],
            release_year: None,
        })
        .collect();

    let MatchDecision::Review { candidates: shown } = decide_by_title("Juego", None, &candidates)
    else {
        panic!("diez candidatos parecidos no pueden resolverse solos");
    };

    assert!(shown.len() <= 5, "la cola no puede escupir diez opciones");
    assert!(
        shown.windows(2).all(|w| w[0].score >= w[1].score),
        "el mejor candidato va primero"
    );
}

#[test]
fn normalizacion() {
    // El empaquetado se va.
    assert_eq!(
        normalize("BioShock Infinite: Complete Edition"),
        "bioshock infinite"
    );
    assert_eq!(normalize("Borderlands 2 Game of the Year"), "borderlands 2");
    // Marcas comerciales, acentos y puntuación, también.
    assert_eq!(
        normalize("DARK SOULS™: REMASTERED"),
        "dark souls remastered"
    );
    assert_eq!(normalize("Ōkami HD"), "okami hd");
    assert_eq!(normalize("Tráiler Park"), "trailer park");
    // Los romanos se unifican para que "III" y "3" sean el mismo juego.
    assert_eq!(normalize("Baldur's Gate III"), "baldur s gate 3");
    assert_eq!(normalize("Final Fantasy VII"), "final fantasy 7");
    // Pero "remastered" se queda: es otro juego, no otro empaquetado.
    assert!(normalize("Dark Souls: Remastered").contains("remastered"));
}

#[test]
fn un_original_y_su_remaster_nunca_se_fusionan_solos() {
    let candidates = vec![
        Candidate {
            igdb_id: 1608,
            name: "Dark Souls".to_owned(),
            alternative_names: vec![],
            release_year: Some(2011),
        },
        Candidate {
            igdb_id: 11133,
            name: "Dark Souls: Remastered".to_owned(),
            alternative_names: vec![],
            release_year: Some(2018),
        },
    ];

    // El original no puede caer en la ficha del remaster ni al revés.
    match decide_by_title("Dark Souls", Some(2011), &candidates) {
        MatchDecision::Auto { igdb_id, .. } => assert_eq!(igdb_id, 1608),
        MatchDecision::Review { .. } => {}
    }
    match decide_by_title("DARK SOULS™: REMASTERED", Some(2018), &candidates) {
        MatchDecision::Auto { igdb_id, .. } => assert_eq!(igdb_id, 11133),
        MatchDecision::Review { .. } => {}
    }
}

#[test]
fn un_ano_incompatible_manda_a_revision_aunque_el_titulo_sea_identico() {
    let candidates = vec![Candidate {
        igdb_id: 250,
        name: "Doom".to_owned(),
        alternative_names: vec![],
        release_year: Some(1993),
    }];

    assert!(matches!(
        decide_by_title("Doom", Some(2016), &candidates),
        MatchDecision::Review { .. }
    ));
}

#[test]
fn los_nombres_alternativos_cuentan() {
    let candidates = vec![Candidate {
        igdb_id: 19686,
        name: "NieR: Automata".to_owned(),
        alternative_names: vec!["ニーア オートマタ".to_owned(), "Nier Automata".to_owned()],
        release_year: Some(2017),
    }];

    assert!(matches!(
        decide_by_title("Nier:Automata™", Some(2017), &candidates),
        MatchDecision::Auto { igdb_id: 19686, .. }
    ));
}
