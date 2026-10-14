//! The skins folder gives the interface only what the contract says.

use std::fs;

use gamelibrarymanager_lib::testing::{
    MAX_SKIN_BYTES, MAX_SKIN_FILES, UserSkinFile, read_skins_dir,
};

fn text(file: &str, text: &str) -> UserSkinFile {
    UserSkinFile {
        file: file.to_owned(),
        text: Some(text.to_owned()),
        error: None,
    }
}

#[test]
fn the_folder_is_made_when_it_does_not_exist() {
    let root = tempfile::tempdir().expect("temporary directory");
    let dir = root.path().join("skins");
    let skins = read_skins_dir(&dir).expect("read the folder");
    assert!(dir.is_dir());
    assert!(skins.files.is_empty());
    assert_eq!(skins.dir, dir.to_string_lossy());
}

#[test]
fn only_the_json_files_of_the_first_level_are_read() {
    let root = tempfile::tempdir().expect("temporary directory");
    let dir = root.path();
    fs::write(dir.join("b.json"), r#"{"id":"b"}"#).unwrap();
    fs::write(dir.join("a.JSON"), r#"{"id":"a"}"#).unwrap();
    fs::write(dir.join("notes.txt"), "not a skin").unwrap();
    fs::create_dir(dir.join("nested")).unwrap();
    fs::write(dir.join("nested/c.json"), r#"{"id":"c"}"#).unwrap();

    let skins = read_skins_dir(dir).expect("read the folder");
    assert_eq!(
        skins.files,
        vec![
            text("a.JSON", r#"{"id":"a"}"#),
            text("b.json", r#"{"id":"b"}"#)
        ]
    );
}

#[test]
fn a_file_that_breaks_a_limit_says_why_and_the_others_continue() {
    let root = tempfile::tempdir().expect("temporary directory");
    let dir = root.path();
    fs::write(
        dir.join("big.json"),
        vec![b' '; MAX_SKIN_BYTES as usize + 1],
    )
    .unwrap();
    fs::write(dir.join("binary.json"), [0xff, 0xfe, 0x00]).unwrap();
    fs::create_dir(dir.join("folder.json")).unwrap();
    fs::write(dir.join("good.json"), "{}").unwrap();

    let skins = read_skins_dir(dir).expect("read the folder");
    let errors: Vec<(&str, Option<&str>)> = skins
        .files
        .iter()
        .map(|f| (f.file.as_str(), f.error.as_deref()))
        .collect();
    assert_eq!(
        errors,
        vec![
            ("big.json", Some("it has more than 64 KB")),
            ("binary.json", Some("it is not UTF-8 text")),
            ("folder.json", Some("it is not a file")),
            ("good.json", None),
        ]
    );
    assert!(
        skins
            .files
            .iter()
            .all(|f| f.error.is_none() || f.text.is_none())
    );
}

#[cfg(unix)]
#[test]
fn a_link_is_not_followed() {
    let root = tempfile::tempdir().expect("temporary directory");
    let secret = root.path().join("secret.txt");
    fs::write(&secret, "do not read me").unwrap();
    let dir = root.path().join("skins");
    fs::create_dir(&dir).unwrap();
    std::os::unix::fs::symlink(&secret, dir.join("link.json")).unwrap();

    let skins = read_skins_dir(&dir).expect("read the folder");
    assert_eq!(skins.files.len(), 1);
    assert_eq!(skins.files[0].text, None);
    assert_eq!(
        skins.files[0].error.as_deref(),
        Some("it is a link: put the file itself in the folder")
    );
}

#[test]
fn no_more_than_the_limit_of_files_is_read() {
    let root = tempfile::tempdir().expect("temporary directory");
    let dir = root.path();
    for i in 0..MAX_SKIN_FILES + 10 {
        fs::write(dir.join(format!("skin-{i:03}.json")), "{}").unwrap();
    }

    let skins = read_skins_dir(dir).expect("read the folder");
    assert_eq!(skins.files.len(), MAX_SKIN_FILES + 10);
    assert_eq!(
        skins.files.iter().filter(|f| f.text.is_some()).count(),
        MAX_SKIN_FILES
    );
    assert!(
        skins.files[MAX_SKIN_FILES..]
            .iter()
            .all(|f| f.error.is_some())
    );
}
