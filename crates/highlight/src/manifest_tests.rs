use super::*;

#[test]
fn reads_a_pack_with_optional_lists_left_out() {
    let pack = parse(
        r#"{"name": "markdown", "version": "0.3.2", "grammars": [
            {"name": "markdown", "extensions": ["md"], "aliases": ["md"]},
            {"name": "markdown_inline"}
        ]}"#,
    )
    .unwrap();
    assert_eq!(pack.name, "markdown");
    assert_eq!(pack.version, "0.3.2");
    assert_eq!(pack.grammars[0].extensions, ["md"]);
    assert_eq!(pack.grammars[1].name, "markdown_inline");
    assert!(pack.grammars[1].filenames.is_empty());
}

#[test]
fn rejects_names_unsafe_as_paths() {
    let pack = |name: &str, version: &str, grammar: &str| {
        format!(
            r#"{{"name": "{name}", "version": "{version}", "grammars": [{{"name": "{grammar}"}}]}}"#
        )
    };
    for json in [
        pack("../x", "1", "x"),
        pack("x", "..", "x"),
        pack("x", "1/2", "x"),
        pack("x", "1", "a.b"),
        pack("X", "1", "x"),
        pack("", "1", "x"),
        r#"{"name": "x", "version": "1", "grammars": []}"#.into(),
        r#"{"name": "x", "version": "1", "grammars": [{"name": "a"}, {"name": "a"}]}"#.into(),
        "{".into(),
    ] {
        assert!(parse(&json).is_err(), "{json}");
    }
}
