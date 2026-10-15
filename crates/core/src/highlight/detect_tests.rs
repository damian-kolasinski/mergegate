use super::*;
use crate::highlight::Grammar;

fn pack(name: &str, extensions: &[&str], filenames: &[&str]) -> Pack {
    Pack {
        name: name.into(),
        version: "1".into(),
        grammars: vec![Grammar {
            name: name.into(),
            extensions: extensions.iter().map(|e| (*e).into()).collect(),
            filenames: filenames.iter().map(|f| (*f).into()).collect(),
            aliases: Vec::new(),
        }],
    }
}

fn detected(paths: &[&str]) -> Vec<(String, usize)> {
    let known = [
        pack("rust", &["rs"], &[]),
        pack("toml", &["toml"], &["Cargo.lock"]),
        pack("markdown", &["md"], &[]),
    ];
    detect(paths.iter().copied(), &known)
        .into_iter()
        .map(|d| (d.pack, d.files))
        .collect()
}

#[test]
fn counts_files_per_pack_most_first() {
    let found = detected(&[
        "src/main.rs",
        "src/lib.rs",
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
    ]);
    assert_eq!(
        found,
        [
            ("rust".into(), 2),
            ("toml".into(), 2),
            ("markdown".into(), 1)
        ]
    );
}

#[test]
fn files_no_pack_highlights_are_left_out() {
    assert_eq!(detected(&["logo.png", "Makefile", ".gitignore"]), []);
}

#[test]
fn nothing_detected_in_an_empty_repo() {
    assert_eq!(detected(&[]), []);
}
