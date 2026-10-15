use super::*;

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";
const ZERO: &str = "0000000000000000000000000000000000000000";

use FileMode::{Executable, Regular, Submodule, Symlink};

fn file(
    path: &str,
    old_path: Option<&str>,
    (old, old_mode): (Option<&str>, Option<FileMode>),
    (new, new_mode): (Option<&str>, Option<FileMode>),
) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        old_path: old_path.map(Into::into),
        old_blob: old.map(Into::into),
        new_blob: new.map(Into::into),
        old_mode,
        new_mode,
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
    let regular = |id| (Some(id), Some(Regular));
    assert_eq!(
        parse_raw(&output).unwrap(),
        [
            file("src/a b.rs", None, regular(A), regular(B)),
            file("logo.png", None, (None, None), regular(A)),
            file("gone", None, regular(B), (None, None)),
            file("new/name.rs", Some("old/name.rs"), regular(A), regular(A)),
        ]
    );
    assert_eq!(parse_raw("").unwrap(), []);
    for broken in [
        format!("100644 100644 {A} {B} M\0a\0"),
        format!(":100644 100644 {A} abc M\0a\0"),
        format!(":100644 100644 {A} {B}\0a\0"),
        format!(":100644 100644 {A} {B} R090\0only-old"),
        format!(":100644 040000 {A} {B} T\0a\0"),
        format!(":100644 {A} {B} M\0a\0"),
    ] {
        assert_eq!(parse_raw(&broken), None, "{broken:?}");
    }
}

#[test]
fn keeps_the_modes_a_diff_of_blobs_leaves_out() {
    let output = format!(
        ":100644 100755 {A} {A} M\0run.sh\0\
         :000000 120000 {ZERO} {A} A\0evil\0\
         :100644 120000 {A} {B} T\0a.txt\0\
         :000000 160000 {ZERO} {B} A\0vendor/dep\0\
         :160000 160000 {A} {B} M\0vendor/other\0"
    );
    assert_eq!(
        parse_raw(&output).unwrap(),
        [
            file(
                "run.sh",
                None,
                (Some(A), Some(Regular)),
                (Some(A), Some(Executable))
            ),
            file("evil", None, (None, None), (Some(A), Some(Symlink))),
            file(
                "a.txt",
                None,
                (Some(A), Some(Regular)),
                (Some(B), Some(Symlink))
            ),
            file("vendor/dep", None, (None, None), (Some(B), Some(Submodule))),
            file(
                "vendor/other",
                None,
                (Some(A), Some(Submodule)),
                (Some(B), Some(Submodule))
            ),
        ]
    );
}
