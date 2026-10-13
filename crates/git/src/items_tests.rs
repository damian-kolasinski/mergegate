use super::*;

fn file(path: &str, old_path: Option<&str>, lines: Option<(u64, u64)>) -> FileStat {
    FileStat {
        path: path.into(),
        old_path: old_path.map(Into::into),
        additions: lines.map(|l| l.0),
        deletions: lines.map(|l| l.1),
    }
}

#[test]
fn parses_numstat_output() {
    let output = "3\t1\tsrc/a b.rs\0-\t-\tlogo.png\0\
                  0\t0\t\0old/name.rs\0new/name.rs\0";
    assert_eq!(
        parse_numstat(output).unwrap(),
        [
            file("src/a b.rs", None, Some((3, 1))),
            file("logo.png", None, None),
            file("new/name.rs", Some("old/name.rs"), Some((0, 0))),
        ]
    );
    assert_eq!(parse_numstat("").unwrap(), []);
    for broken in ["3\t1\0", "x\t1\ta\0", "0\t0\t\0only-old"] {
        assert_eq!(parse_numstat(broken), None, "{broken:?}");
    }
}
