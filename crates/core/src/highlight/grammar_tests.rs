use super::*;

fn grammar(name: &str, extensions: &[&str], filenames: &[&str]) -> Grammar {
    Grammar {
        name: name.into(),
        extensions: extensions.iter().map(|e| (*e).into()).collect(),
        filenames: filenames.iter().map(|f| (*f).into()).collect(),
        aliases: Vec::new(),
    }
}

fn packs() -> Vec<Pack> {
    vec![
        Pack {
            name: "json".into(),
            version: "1".into(),
            grammars: vec![grammar("json", &["json"], &[".babelrc"])],
        },
        Pack {
            name: "toml".into(),
            version: "1".into(),
            grammars: vec![grammar("toml", &["toml"], &["Cargo.lock"])],
        },
        Pack {
            name: "typescript".into(),
            version: "1".into(),
            grammars: vec![
                grammar("typescript", &["ts"], &[]),
                grammar("tsx", &["tsx"], &[]),
            ],
        },
    ]
}

fn found(path: &str) -> Option<String> {
    let packs = packs();
    grammar_for(path, &packs).map(|(p, g)| format!("{}/{}", p.name, g.name))
}

#[test]
fn matches_the_extension_of_the_file_name() {
    assert_eq!(found("ui/app.tsx").as_deref(), Some("typescript/tsx"));
    assert_eq!(
        found("src/a.b/main.ts").as_deref(),
        Some("typescript/typescript")
    );
    assert_eq!(found("data.JSON").as_deref(), Some("json/json"));
}

#[test]
fn matches_a_file_name_before_its_extension() {
    assert_eq!(found("Cargo.lock").as_deref(), Some("toml/toml"));
    assert_eq!(found("web/.babelrc").as_deref(), Some("json/json"));
}

#[test]
fn takes_no_extension_from_a_hidden_files_name_or_a_directory() {
    assert_eq!(found(".json"), None);
    assert_eq!(found("conf.json/README"), None);
    assert_eq!(found("Makefile"), None);
    assert_eq!(found("deps/other.lock"), None);
}
