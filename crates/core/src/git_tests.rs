use super::*;

#[test]
fn parses_git_versions() {
    let cases = [
        ("git version 2.50.1 (Apple Git-155)", Some((2, 50))),
        ("git version 2.38.0\n", Some((2, 38))),
        ("git version 2.45.2.windows.1", Some((2, 45))),
        ("git version 3.0", Some((3, 0))),
        ("hub version 2.14.2", None),
        ("git version", None),
    ];
    for (output, expected) in cases {
        assert_eq!(parse_version(output), expected, "{output:?}");
    }
}

#[test]
fn secrets_stay_out_of_debug_output() {
    let remote = Remote {
        url: "https://github.com/o/r.git".into(),
        auth: Secret::new("Authorization: Basic c2VjcmV0".into()),
    };
    let shown = format!("{remote:?}");
    assert!(!shown.contains("c2VjcmV0"), "{shown}");
    assert!(shown.contains("Secret(..)"), "{shown}");
}
