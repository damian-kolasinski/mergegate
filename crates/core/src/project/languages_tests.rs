use super::*;
use crate::fake::{FakeGit, pack};

const ROOT: &str = "/src/app";

fn git() -> FakeGit {
    FakeGit::default().repo(ROOT, &["main"], None).files(
        ROOT,
        &["src/main.rs", "src/lib.rs", "Cargo.toml", "logo.png"],
    )
}

fn languages(installed: &[&str], chosen: Option<&[&str]>) -> Languages {
    Languages {
        known: vec![pack("rs", "1"), pack("toml", "1"), pack("md", "1")],
        installed: installed.iter().map(|n| (*n).into()).collect(),
        chosen: chosen.map(|c| c.iter().map(|n| (*n).into()).collect()),
    }
}

fn chosen(languages: &Languages) -> Result<Chosen, ProjectError> {
    choose(&git(), Path::new(ROOT), "main", languages)
}

#[test]
fn asks_to_install_the_packs_the_repo_uses_that_are_missing() {
    let Chosen::Decide(decision) = chosen(&languages(&[], None)).unwrap() else {
        panic!("installed without asking");
    };
    assert_eq!(decision.flag, "--languages");
    assert!(
        decision.question.contains("rs (2 files), toml (1 file)"),
        "{}",
        decision.question
    );
    let values: Vec<_> = decision.options.iter().map(|o| o.value.as_str()).collect();
    assert_eq!(values, ["rs,toml", NONE]);
    assert_eq!(decision.default.as_deref(), Some("rs,toml"));
}

#[test]
fn installed_packs_are_not_asked_for() {
    let Chosen::Decide(decision) = chosen(&languages(&["rs"], None)).unwrap() else {
        panic!("installed without asking");
    };
    assert_eq!(decision.default.as_deref(), Some("toml"));
}

#[test]
fn nothing_to_ask_when_every_pack_used_is_installed() {
    let found = chosen(&languages(&["rs", "toml"], None)).unwrap();
    assert!(matches!(found, Chosen::Install(names) if names.is_empty()));
}

#[test]
fn nothing_to_ask_when_mg_knows_no_packs() {
    let languages = Languages::default();
    let found = chosen(&languages).unwrap();
    assert!(matches!(found, Chosen::Install(names) if names.is_empty()));
}

#[test]
fn the_operators_choice_is_installed_as_given() {
    let found = chosen(&languages(&[], Some(&["md"]))).unwrap();
    assert!(matches!(found, Chosen::Install(names) if names == ["md"]));
    let found = chosen(&languages(&[], Some(&[]))).unwrap();
    assert!(matches!(found, Chosen::Install(names) if names.is_empty()));
}

#[test]
fn a_choice_naming_an_unknown_pack_fails() {
    let err = chosen(&languages(&[], Some(&["rs", "cobol"])))
        .err()
        .unwrap();
    assert!(matches!(
        err,
        ProjectError::InvalidValue { key: "languages", value, .. } if value == "cobol"
    ));
}

#[test]
fn missing_lists_the_used_packs_not_installed_most_files_first() {
    let known = [pack("toml", "1"), pack("rs", "1")];
    let found = missing(&git(), Path::new(ROOT), "main", &known, &[]).unwrap();
    let found: Vec<_> = found.iter().map(|d| (d.pack.as_str(), d.files)).collect();
    assert_eq!(found, [("rs", 2), ("toml", 1)]);
}
