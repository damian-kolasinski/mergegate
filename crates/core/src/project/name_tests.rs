use super::*;

#[test]
fn parse_accepts_only_plain_names() {
    let long = "a".repeat(MAX_LEN + 1);
    let cases = [
        ("app", Ok(())),
        ("my-app-2", Ok(())),
        ("9", Ok(())),
        ("", Err(NameError::Empty)),
        (long.as_str(), Err(NameError::TooLong(long.clone()))),
        ("App", Err(NameError::Invalid("App".into()))),
        ("my_app", Err(NameError::Invalid("my_app".into()))),
        ("-app", Err(NameError::Invalid("-app".into()))),
        ("app-", Err(NameError::Invalid("app-".into()))),
        ("app#1", Err(NameError::Invalid("app#1".into()))),
    ];
    for (name, expected) in cases {
        assert_eq!(ProjectName::parse(name).map(|_| ()), expected, "{name:?}");
    }
}

#[test]
fn derive_turns_directory_names_into_names() {
    let cases = [
        ("app", Some("app")),
        ("MyApp", Some("myapp")),
        ("my_app.rs", Some("my-app-rs")),
        ("--app--", Some("app")),
        ("app.git", Some("app")),
        ("Zażółć app", Some("za-app")),
        ("___", None),
        ("", None),
        (
            "a-very-long-repository-name-that-goes-on",
            Some("a-very-long-repository-name-that"),
        ),
        (
            "a-very-long-repository-name-tha-x",
            Some("a-very-long-repository-name-tha"),
        ),
    ];
    for (dir, expected) in cases {
        assert_eq!(
            ProjectName::derive(dir).as_ref().map(ProjectName::as_str),
            expected,
            "{dir:?}"
        );
    }
}
