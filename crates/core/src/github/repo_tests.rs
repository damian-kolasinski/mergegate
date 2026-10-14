use super::*;

#[test]
fn parses_owner_and_name() {
    let repo = GithubRepo::parse("damian-kolasinski/merge.gate_2").unwrap();
    assert_eq!(repo.owner(), "damian-kolasinski");
    assert_eq!(repo.name(), "merge.gate_2");
    assert_eq!(repo.to_string(), "damian-kolasinski/merge.gate_2");
    assert_eq!(GithubRepo::parse("o/r.git"), GithubRepo::parse("o/r"));
}

#[test]
fn rejects_what_github_would() {
    for bad in [
        "", "o", "o/", "/r", "o/r/x", "-o/r", "o-/r", "o--p/r", "o p/r", "o/..", "o/.", "o/r s",
        "ö/r",
    ] {
        assert_eq!(GithubRepo::parse(bad), None, "{bad:?}");
    }
    assert!(GithubRepo::parse(&format!("{}/r", "o".repeat(40))).is_none());
}

#[test]
fn reads_github_remote_urls() {
    let repo = GithubRepo::parse("o/r").unwrap();
    for url in [
        "git@github.com:o/r.git",
        "git@github.com:o/r",
        "ssh://git@github.com/o/r.git",
        "ssh://git@github.com:22/o/r.git",
        "https://github.com/o/r",
        "https://github.com/o/r.git",
        "https://github.com/o/r/",
        "https://user@github.com/o/r.git",
    ] {
        assert_eq!(GithubRepo::from_url(url), Some(repo.clone()), "{url}");
    }
}

#[test]
fn ignores_other_hosts() {
    for url in [
        "git@gitlab.com:o/r.git",
        "https://github.example.com/o/r",
        "https://notgithub.com/o/r",
        "/src/app",
        "../app.git",
        "file:///src/app",
    ] {
        assert_eq!(GithubRepo::from_url(url), None, "{url}");
    }
}
