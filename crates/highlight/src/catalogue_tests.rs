use std::collections::HashSet;

use mergegate_core::highlight::grammar_for;

use super::*;

#[test]
fn names_every_grammar_once() {
    let packs = known();
    assert!(packs.len() >= 15);
    let mut grammars = HashSet::new();
    for pack in &packs {
        for g in &pack.grammars {
            assert!(grammars.insert(g.name.clone()), "{} twice", g.name);
        }
    }
}

#[test]
fn finds_the_pack_for_common_files() {
    let packs = known();
    for (path, pack, grammar) in [
        ("src/main.rs", "rust", "rust"),
        ("ui/app.tsx", "typescript", "tsx"),
        ("Cargo.toml", "toml", "toml"),
        ("README.md", "markdown", "markdown"),
        (".github/workflows/ci.yml", "yaml", "yaml"),
    ] {
        let (p, g) = grammar_for(path, &packs).unwrap();
        assert_eq!(
            (p.name.as_str(), g.name.as_str()),
            (pack, grammar),
            "{path}"
        );
    }
}
