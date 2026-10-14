use super::*;

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";
const ZERO: &str = "0000000000000000000000000000000000000000";

fn file(path: &str, old_path: Option<&str>, old: Option<&str>, new: Option<&str>) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        old_path: old_path.map(Into::into),
        old_blob: old.map(Into::into),
        new_blob: new.map(Into::into),
    }
}

#[test]
fn parses_raw_output() {
    let output = format!(
        ":100644 100644 {A} {B} M\0src/a b.rs\0\
         :000000 100644 {ZERO} {A} A\0logo.png\0\
         :100644 000000 {B} {ZERO} D\0gone\0\
         :100644 100644 {A} {A} R100\0old/name.rs\0new/name.rs\0"
    );
    assert_eq!(
        parse_raw(&output).unwrap(),
        [
            file("src/a b.rs", None, Some(A), Some(B)),
            file("logo.png", None, None, Some(A)),
            file("gone", None, Some(B), None),
            file("new/name.rs", Some("old/name.rs"), Some(A), Some(A)),
        ]
    );
    assert_eq!(parse_raw("").unwrap(), []);
    for broken in [
        format!("100644 100644 {A} {B} M\0a\0"),
        format!(":100644 100644 {A} abc M\0a\0"),
        format!(":100644 100644 {A} {B}\0a\0"),
        format!(":100644 100644 {A} {B} R090\0only-old"),
    ] {
        assert_eq!(parse_raw(&broken), None, "{broken:?}");
    }
}
