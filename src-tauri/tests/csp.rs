//! The fonts of the skins come from the application, never from the network.
//!
//! Without `font-src`, the CSP takes `default-src 'self'` for the fonts, and
//! that is already correct. The directive is written explicitly so that a
//! change to `default-src` does not open the fonts by accident, and this test
//! makes sure that nobody adds a remote origin to it: a skin that needs a font
//! from a CDN would make the look of the application depend on the network.

use std::path::PathBuf;

fn csp() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(root.join("tauri.conf.json")).expect("read tauri.conf.json");
    let config: serde_json::Value = serde_json::from_str(&raw).expect("tauri.conf.json is JSON");
    config["app"]["security"]["csp"]
        .as_str()
        .expect("the CSP is a string")
        .to_owned()
}

#[test]
fn fonts_load_only_from_the_application() {
    let csp = csp();
    let font_src: Vec<&str> = csp
        .split(';')
        .map(str::trim)
        .find_map(|directive| directive.strip_prefix("font-src "))
        .expect("the CSP declares font-src")
        .split_whitespace()
        .collect();
    assert_eq!(font_src, vec!["'self'"]);
}
