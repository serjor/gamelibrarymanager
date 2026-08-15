//! The "done when" of phase 4, over 52 real titles exactly as the stores write
//! them.
//!
//! The criterion is not symmetrical and that is deliberate: **zero false
//! positives**. A game that stays unmatched appears in the review queue and the
//! user corrects it with two clicks; two different games merged make the user
//! lose the status and the notes of one of the two, and with no message.

use domain::matching::{Candidate, MatchDecision, decide_by_title, normalize};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    title: String,
    year: Option<i32>,
    /// An expected `igdb_id`, or the string "review".
    expected: serde_json::Value,
    candidates: Vec<Candidate>,
}

fn corpus() -> Vec<Case> {
    serde_json::from_str(include_str!("fixtures/corpus.json")).expect("corpus ilegible")
}

#[test]
fn no_false_positive_and_sufficient_precision() {
    let cases = corpus();
    let mut hits = 0usize;
    let mut false_positives = Vec::new();
    let mut unmatched = Vec::new();

    for case in &cases {
        let decision = decide_by_title(&case.title, case.year, &case.candidates);
        let expected = case.expected.as_i64();

        match (&decision, expected) {
            (MatchDecision::Auto { igdb_id, .. }, Some(wanted)) if *igdb_id == wanted => {
                hits += 1;
            }
            // It linked something different from the expected record, or it
            // linked where it had to ask. This is the error that is not
            // acceptable.
            (MatchDecision::Auto { igdb_id, .. }, _) => {
                false_positives.push(format!(
                    "\"{}\" linked with {} and had to be {:?}",
                    case.title, igdb_id, case.expected
                ));
            }
            (MatchDecision::Review { .. }, None) => hits += 1,
            (MatchDecision::Review { .. }, Some(_)) => unmatched.push(case.title.clone()),
        }
    }

    assert!(
        false_positives.is_empty(),
        "false positives, which is the only error that is not acceptable:\\n{}",
        false_positives.join("\n")
    );

    let precision = hits as f64 / cases.len() as f64;
    println!(
        "corpus: {hits}/{} correctos ({:.1}%), {} sin emparejar, 0 falsos positivos",
        cases.len(),
        precision * 100.0,
        unmatched.len()
    );
    assert!(
        precision >= 0.95,
        "precision {:.1}% over {} cases (minimum 95%). Unmatched:\\n{}",
        precision * 100.0,
        cases.len(),
        unmatched.join("\\n")
    );
}

#[test]
fn the_candidates_of_the_queue_come_sorted_and_limited() {
    let candidates: Vec<Candidate> = (1..=10)
        .map(|i| Candidate {
            igdb_id: i,
            name: format!("Juego {i}"),
            alternative_names: vec![],
            release_year: None,
            cover_url: None,
            slug: None,
        })
        .collect();

    let MatchDecision::Review { candidates: shown } = decide_by_title("Juego", None, &candidates)
    else {
        panic!("diez candidates parecidos no pueden resolverse solos");
    };

    assert!(shown.len() <= 5, "la queue no puede escupir diez opciones");
    assert!(
        shown.windows(2).all(|w| w[0].score >= w[1].score),
        "el mejor candidate va primero"
    );
}

#[test]
fn normalizacion() {
    // The packaging goes away.
    assert_eq!(
        normalize("BioShock Infinite: Complete Edition"),
        "bioshock infinite"
    );
    assert_eq!(normalize("Borderlands 2 Game of the Year"), "borderlands 2");
    // Trade marks, accents and punctuation also go away.
    assert_eq!(
        normalize("DARK SOULS™: REMASTERED"),
        "dark souls remastered"
    );
    assert_eq!(normalize("Ōkami HD"), "okami hd");
    assert_eq!(normalize("Tráiler Park"), "trailer park");
    // The Roman numerals become the same so that "III" and "3" are one game.
    assert_eq!(normalize("Baldur's Gate III"), "baldur s gate 3");
    assert_eq!(normalize("Final Fantasy VII"), "final fantasy 7");
    // But "remastered" stays: it is a different game, not different packaging.
    assert!(normalize("Dark Souls: Remastered").contains("remastered"));
}

#[test]
fn an_original_and_its_remaster_never_merge_alone() {
    let candidates = vec![
        Candidate {
            igdb_id: 1608,
            name: "Dark Souls".to_owned(),
            alternative_names: vec![],
            release_year: Some(2011),
            cover_url: None,
            slug: None,
        },
        Candidate {
            igdb_id: 11133,
            name: "Dark Souls: Remastered".to_owned(),
            alternative_names: vec![],
            release_year: Some(2018),
            cover_url: None,
            slug: None,
        },
    ];

    // The original cannot fall into the record of the remaster or the opposite.
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
fn an_incompatible_year_sends_to_review_even_with_an_identical_title() {
    let candidates = vec![Candidate {
        igdb_id: 250,
        name: "Doom".to_owned(),
        alternative_names: vec![],
        release_year: Some(1993),
        cover_url: None,
        slug: None,
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
        cover_url: None,
        slug: None,
    }];

    assert!(matches!(
        decide_by_title("Nier:Automata™", Some(2017), &candidates),
        MatchDecision::Auto { igdb_id: 19686, .. }
    ));
}
