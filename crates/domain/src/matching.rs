//! Identidad de juegos: decidir cuándo dos títulos son el mismo juego.
//!
//! Es la parte difícil del producto y la única que no puede fallar en silencio.
//! La regla que gobierna todo el módulo: **un duplicado visible molesta, una
//! fusión errónea hace perder datos del usuario**. Por eso, ante la duda, la
//! decisión es mandar a revisión y no enlazar.
//!
//! Puro y sin IO: se prueba con un corpus de títulos reales y sin red.

use serde::{Deserialize, Serialize};

/// Parecido mínimo para enlazar sin preguntar.
pub const AUTO_THRESHOLD: f64 = 0.90;

/// Distancia mínima entre el mejor candidato y el segundo. Si dos fichas se
/// parecen igual de bien, ninguna gana: casi siempre son un juego y su
/// remaster, o dos entregas de la misma saga.
pub const AMBIGUITY_MARGIN: f64 = 0.06;

/// Confianza de un enlace hecho sin base de metadatos, agrupando solo por
/// título normalizado idéntico.
///
/// No es una identidad y por eso no vale 1.0: eso queda reservado al
/// identificador externo. Es lo máximo que se puede afirmar sin IGDB, se queda
/// justo en el umbral automático, y el primer emparejamiento con IGDB lo
/// sustituye.
pub const LOCAL_TITLE_CONFIDENCE: f64 = AUTO_THRESHOLD;

/// Un candidato de la base de metadatos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub igdb_id: i64,
    pub name: String,
    #[serde(default)]
    pub alternative_names: Vec<String>,
    #[serde(default)]
    pub release_year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredCandidate {
    pub igdb_id: i64,
    pub name: String,
    pub score: f64,
}

/// Qué hacer con una entrada de tienda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchDecision {
    /// Enlace automático. `confidence` 1.0 solo cuando viene de un
    /// identificador externo, nunca de un parecido de texto.
    Auto { igdb_id: i64, confidence: f64 },
    /// A la cola de revisión, con lo que se ha encontrado para que el usuario
    /// elija sin tener que buscarlo él.
    Review { candidates: Vec<ScoredCandidate> },
}

/// Emparejamiento por identificador externo: el appid de Steam contra
/// `external_games` de IGDB. Es exacto, así que no se puntúa ni se pregunta.
pub fn decide_by_external_id(igdb_id: i64) -> MatchDecision {
    MatchDecision::Auto {
        igdb_id,
        confidence: 1.0,
    }
}

/// Emparejamiento por título, para las tiendas que no tienen identificador
/// cruzado con IGDB.
pub fn decide_by_title(
    store_title: &str,
    store_year: Option<i32>,
    candidates: &[Candidate],
) -> MatchDecision {
    let needle = normalize(store_title);

    let mut scored: Vec<ScoredCandidate> = candidates
        .iter()
        .map(|candidate| ScoredCandidate {
            igdb_id: candidate.igdb_id,
            name: candidate.name.clone(),
            score: best_score(&needle, candidate),
        })
        .collect();

    scored.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.igdb_id.cmp(&b.igdb_id)));
    scored.truncate(5);

    let Some(best) = scored.first() else {
        return MatchDecision::Review { candidates: scored };
    };

    let year_ok = match (store_year, year_of(candidates, best.igdb_id)) {
        // Un año de diferencia es normal: relanzamientos, regiones, y la fecha
        // que guarda la tienda no siempre es la de salida.
        (Some(a), Some(b)) => (a - b).abs() <= 1,
        _ => true,
    };

    let unambiguous = scored
        .get(1)
        .is_none_or(|second| best.score - second.score >= AMBIGUITY_MARGIN);

    if best.score >= AUTO_THRESHOLD && year_ok && unambiguous {
        MatchDecision::Auto {
            igdb_id: best.igdb_id,
            confidence: best.score,
        }
    } else {
        MatchDecision::Review { candidates: scored }
    }
}

fn best_score(needle: &str, candidate: &Candidate) -> f64 {
    std::iter::once(&candidate.name)
        .chain(candidate.alternative_names.iter())
        .map(|name| title_similarity(needle, &normalize(name)))
        .fold(0.0, f64::max)
}

fn year_of(candidates: &[Candidate], igdb_id: i64) -> Option<i32> {
    candidates
        .iter()
        .find(|c| c.igdb_id == igdb_id)
        .and_then(|c| c.release_year)
}

/// Sufijos que solo describen el empaquetado y no un juego distinto.
///
/// `remastered`, `remake`, `redux` y `enhanced` **no** están en la lista a
/// propósito: son juegos diferentes con ficha propia, y borrarlos fusionaría un
/// original con su reedición.
const PACKAGING_SUFFIXES: &[&str] = &[
    "game of the year edition",
    "game of the year",
    "goty edition",
    "goty",
    "definitive edition",
    "complete edition",
    "complete pack",
    "deluxe edition",
    "ultimate edition",
    "gold edition",
    "standard edition",
    "premium edition",
    "digital edition",
];

/// Normaliza un título para poder compararlo: minúsculas, sin marcas
/// comerciales, sin acentos, sin puntuación, romanos a arábigos y sin sufijos
/// de empaquetado.
pub fn normalize(title: &str) -> String {
    let mut text: String = title
        .to_lowercase()
        .chars()
        .map(deaccent)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();

    text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    // Los sufijos se quitan en orden: el más largo primero, para que
    // "game of the year edition" no se coma solo su cola.
    loop {
        let before = text.len();
        for suffix in PACKAGING_SUFFIXES {
            if let Some(stripped) = text.strip_suffix(suffix) {
                text = stripped.trim().to_owned();
            }
        }
        if text.len() == before {
            break;
        }
    }

    text.split(' ')
        .map(|token| roman_to_arabic(token).unwrap_or_else(|| token.to_owned()))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn deaccent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ō' | 'ǒ' => 'o',
        'ū' | 'ǔ' => 'u',
        'ā' | 'ǎ' => 'a',
        'ē' | 'ě' => 'e',
        'ī' | 'ǐ' => 'i',
        'ñ' => 'n',
        'ç' => 'c',
        other => other,
    }
}

/// Solo números romanos sueltos y hasta el XX: suficiente para las sagas y sin
/// riesgo de convertir palabras como "mix" o "civil".
fn roman_to_arabic(token: &str) -> Option<String> {
    const ROMAN: [(&str, u8); 20] = [
        ("i", 1),
        ("ii", 2),
        ("iii", 3),
        ("iv", 4),
        ("v", 5),
        ("vi", 6),
        ("vii", 7),
        ("viii", 8),
        ("ix", 9),
        ("x", 10),
        ("xi", 11),
        ("xii", 12),
        ("xiii", 13),
        ("xiv", 14),
        ("xv", 15),
        ("xvi", 16),
        ("xvii", 17),
        ("xviii", 18),
        ("xix", 19),
        ("xx", 20),
    ];
    ROMAN
        .iter()
        .find(|(roman, _)| *roman == token)
        .map(|(_, value)| value.to_string())
}

/// Parecido entre dos títulos ya normalizados.
///
/// Sobre el parecido textual se impone una regla de dominio: **si los números
/// no coinciden, no es el mismo juego**. `Portal` y `Portal 2` comparten casi
/// todos sus trigramas, y sin esta regla el margen de ambigüedad manda a
/// revisión toda saga numerada; con ella, el número de entrega pesa lo que
/// pesa de verdad.
pub fn title_similarity(a: &str, b: &str) -> f64 {
    let base = similarity(a, b);
    if numeric_tokens(a) == numeric_tokens(b) {
        base
    } else {
        base.min(0.5)
    }
}

fn numeric_tokens(text: &str) -> Vec<u32> {
    let mut numbers: Vec<u32> = text
        .split(' ')
        .filter_map(|token| token.parse::<u32>().ok())
        .collect();
    numbers.sort_unstable();
    numbers
}

/// Coeficiente de Dice sobre trigramas de caracteres.
///
/// Se prefiere a la distancia de edición porque castiga menos las palabras
/// añadidas o quitadas —que es lo que hacen los títulos de tienda— y más el
/// texto realmente distinto.
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let left = trigrams(a);
    let right = trigrams(b);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let mut shared = 0usize;
    let mut remaining = right.clone();
    for trigram in &left {
        if let Some(pos) = remaining.iter().position(|t| t == trigram) {
            remaining.remove(pos);
            shared += 1;
        }
    }

    (2.0 * shared as f64) / (left.len() + right.len()) as f64
}

fn trigrams(text: &str) -> Vec<[char; 3]> {
    let padded: Vec<char> = format!("  {text} ").chars().collect();
    padded.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}
