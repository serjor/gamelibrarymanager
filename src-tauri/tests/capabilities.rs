//! Each link that the interface offers must be permitted in the capability.
//!
//! This test exists because the defect came in two times.
//! `opener:allow-open-url` enables the command but **gives scope to no
//! address**, thus during all of phase 3 the links of the Steam setup screen
//! were broken and nobody saw it: in a container with no browser nobody clicks
//! them. And when the scope was added, a `https://steamid.io/*` came in, which
//! does not agree with `https://steamid.io` because the patterns are compared
//! against the string exactly as it is, with no normalisation and with no last
//! slash.
//!
//! The suite saw neither of the two. Now it does.
//!
//! The second test does not examine whether there are extra patterns, although
//! that would be symmetrical: there are addresses — the page of a game in its
//! store — that are built with data and appear as a literal in no place, and
//! GOG sends one of them in its own answer. What you can demand, and what
//! really protects, is that no pattern opens a complete host.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The URL patterns permitted in `capabilities/default.json`.
fn permitted_patterns() -> Vec<String> {
    let raw_value = std::fs::read_to_string(root().join("capabilities/default.json"))
        .expect("read the capability");
    let capacidad: serde_json::Value =
        serde_json::from_str(&raw_value).expect("the capability must be valid JSON");

    capacidad["permissions"]
        .as_array()
        .expect("permissions is a list")
        .iter()
        .filter(|permiso| permiso["identifier"] == "opener:allow-open-url")
        .flat_map(|permiso| {
            permiso["allow"]
                .as_array()
                .expect("allow is a list")
                .iter()
                .filter_map(|entry| entry["url"].as_str().map(str::to_owned))
        })
        .collect()
}

/// The constant addresses that the interface can give to `openUrl`.
///
/// They are read from the code itself and not kept in a separate list: a
/// separate list becomes old exactly when it is important, which is when
/// somebody adds a new link.
///
/// Only `src/` is examined. To go through the connectors too would be
/// attractive, but their literals are mostly endpoints that the program
/// **calls**, not pages that the user **opens**, and from outside you cannot
/// tell them apart: `https://api.gog.com` must not be permitted and would
/// appear in the same way. The addresses that are opened and are not constants
/// are covered in the test below, with examples.
fn interface_urls() -> BTreeSet<String> {
    let mut urls = BTreeSet::new();
    walk(&root().join("../src/features"), &mut urls);
    assert!(
        !urls.is_empty(),
        "something is wrong in the test if it finds no link"
    );
    urls
}

fn walk(dir: &Path, urls: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, urls);
            continue;
        }
        let is_source = path
            .extension()
            .is_some_and(|ext| ext == "tsx" || ext == "ts");
        let is_test = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(".test."));
        if !is_source || is_test {
            continue;
        }
        let Ok(code) = std::fs::read_to_string(&path) else {
            continue;
        };
        urls.extend(extract_urls(&code));
    }
}

/// Takes the `"https://…"` strings out of the code. This is sufficient: in this
/// project the addresses are literal constants, they are never built in parts.
fn extract_urls(code: &str) -> Vec<String> {
    code.match_indices("\"https://")
        .filter_map(|(start, _)| {
            let rest = &code[start + 1..];
            rest.find('"').map(|end| rest[..end].to_owned())
        })
        .collect()
}

#[test]
fn every_interface_link_is_permitted() {
    let patterns: Vec<glob::Pattern> = permitted_patterns()
        .iter()
        .map(|p| glob::Pattern::new(p).expect("a valid glob pattern"))
        .collect();

    for url in interface_urls() {
        assert!(
            patterns.iter().any(|pattern| pattern.matches(&url)),
            "the interface links to {url} but no pattern of \
             capabilities/default.json permits it: a click would give \
             \"Not allowed to open url\""
        );
    }
}

#[test]
fn the_pages_of_a_copy_and_of_a_record_are_permitted() {
    // These are not constants: the connector builds the Steam address with the
    // appid, the store itself sends the GOG address in its answer and the IGDB
    // address comes from the slug of the candidate. There is no literal to
    // follow, thus they are examined with real examples — the same examples that
    // appear in the fixtures.
    let examples = [
        "https://store.steampowered.com/app/292030",
        "https://www.gog.com/game/the_witcher_2",
        "https://www.igdb.com/games/the-witcher-3-wild-hunt",
        // The ITAD address has two path parts after the slug, and `*` does not
        // cross a slash: thus its pattern is the only one with `**`.
        "https://isthereanydeal.com/game/disco-elysium/info/",
    ];

    let patterns: Vec<glob::Pattern> = permitted_patterns()
        .iter()
        .map(|p| glob::Pattern::new(p).expect("a valid glob pattern"))
        .collect();

    for url in examples {
        assert!(
            patterns.iter().any(|pattern| pattern.matches(url)),
            "the review queue opens addresses such as {url} and no pattern of \
             capabilities/default.json permits it"
        );
    }
}

#[test]
fn no_pattern_opens_a_complete_host() {
    // A permission that is unnecessary is scope given away, and the real way to
    // give it away is a wildcard in the host: `https://*` is
    // `allow-default-urls` with a different name, and `https://*.something.com`
    // opens each subdomain that somebody registers. In the path the wildcard is
    // necessary, because there are addresses built with the identifier of each
    // game.
    for pattern in permitted_patterns() {
        let host = pattern
            .strip_prefix("https://")
            .unwrap_or_else(|| panic!("{pattern} must be https"))
            .split('/')
            .next()
            .unwrap_or_default();

        assert!(
            !host.contains('*') && !host.contains('?'),
            "capabilities/default.json permits {pattern}: the host cannot carry a \
             wildcard, or the scope limits nothing"
        );
    }
}
