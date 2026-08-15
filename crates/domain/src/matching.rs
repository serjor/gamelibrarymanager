//! Game identity: to decide when two titles are the same game.
//!
//! This is the difficult part of the product, and the only part that cannot
//! fail quietly. The rule that controls all of the module: **a duplicate that
//! you see is a nuisance, an incorrect merge loses data of the user**. Thus,
//! when there is doubt, the decision is to send to review and not to link.
//!
//! Pure and with no IO: it is tested with a corpus of real titles and with no
//! network.

use serde::{Deserialize, Serialize};

/// The minimum similarity to link without a question.
pub const AUTO_THRESHOLD: f64 = 0.90;

/// The minimum distance between the best candidate and the second. If two
/// records are equally similar, neither one wins: almost always they are a game
/// and its remaster, or two parts of the same series.
pub const AMBIGUITY_MARGIN: f64 = 0.06;

/// The confidence of a link made with no metadata database, which groups only
/// by an identical normalised title.
///
/// It is not an identity and thus it is not 1.0: that value is kept for the
/// external identifier. It is the most that you can declare without IGDB, it
/// stays exactly at the automatic threshold, and the first match with IGDB
/// replaces it.
pub const LOCAL_TITLE_CONFIDENCE: f64 = AUTO_THRESHOLD;

/// A candidate from the metadata database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub igdb_id: i64,
    pub name: String,
    #[serde(default)]
    pub alternative_names: Vec<String>,
    #[serde(default)]
    pub release_year: Option<i32>,
    #[serde(default)]
    pub cover_url: Option<String>,
    /// The identifier with which IGDB publishes its record. You use it to go
    /// and look at the record.
    #[serde(default)]
    pub slug: Option<String>,
}

/// A candidate with a score, with sufficient data for a person to tell it from
/// another candidate without they leave the application.
///
/// The algorithm does not use the year and the cover: they come to this point
/// because when two candidates are equal — and they are equal frequently,
/// because IGDB has duplicate records and editions that normalise to the same
/// text — the only difference that you see is the cover and the date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredCandidate {
    pub igdb_id: i64,
    pub name: String,
    pub score: f64,
    #[serde(default)]
    pub release_year: Option<i32>,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
}

/// What to do with a store entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchDecision {
    /// An automatic link. `confidence` is 1.0 only when it comes from an
    /// external identifier, never from a similarity of text.
    Auto { igdb_id: i64, confidence: f64 },
    /// To the review queue, with the candidates found, so that the user can
    /// select one without they must search.
    Review { candidates: Vec<ScoredCandidate> },
}

/// A match by external identifier: the Steam appid against `external_games` of
/// IGDB. It is exact, thus there is no score and no question.
pub fn decide_by_external_id(igdb_id: i64) -> MatchDecision {
    MatchDecision::Auto {
        igdb_id,
        confidence: 1.0,
    }
}

/// A match by title, for the stores that have no identifier in common with
/// IGDB.
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
            release_year: candidate.release_year,
            cover_url: candidate.cover_url.clone(),
            slug: candidate.slug.clone(),
        })
        .collect();

    scored.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.igdb_id.cmp(&b.igdb_id)));
    scored.truncate(5);

    let Some(best) = scored.first() else {
        return MatchDecision::Review { candidates: scored };
    };

    let year_ok = match (store_year, year_of(candidates, best.igdb_id)) {
        // A difference of one year is usual: new releases, regions, and the
        // date that the store keeps is not always the release date.
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

/// The suffixes that describe only the packaging and not a different game.
///
/// `remastered`, `remake`, `redux` and `enhanced` are **not** in the list, and
/// that is deliberate: they are different games with a record of their own, and
/// to remove them would merge an initial game with its new edition.
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

/// Normalises a title so that you can compare it: lower case, no trade marks,
/// no accents, no punctuation, Roman numerals to Arabic numerals and no
/// packaging suffixes.
pub fn normalize(title: &str) -> String {
    let mut text: String = title
        .to_lowercase()
        .chars()
        .map(deaccent)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();

    text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    // The suffixes are removed in order, the longest first, so that
    // "game of the year edition" does not lose only its last part.
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

/// Only Roman numerals that are alone, and only to XX: sufficient for the
/// series and with no risk that words such as "mix" or "civil" change.
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

/// The similarity between two titles that are already normalised.
///
/// A domain rule controls the similarity of the text: **if the numbers are not
/// the same, it is not the same game**. `Portal` and `Portal 2` share almost
/// all of their trigrams, and without this rule the ambiguity margin sends
/// every numbered series to review. With the rule, the number of the part has
/// the weight that it must have.
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

/// The Dice coefficient on trigrams of characters.
///
/// It is preferable to the edit distance because it gives a smaller penalty to
/// words that are added or removed — which is what store titles do — and a
/// larger penalty to text that is really different.
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
