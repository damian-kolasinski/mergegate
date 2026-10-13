use super::*;

#[test]
fn accepts_what_git_accepts() {
    let valid = [
        "main",
        "fix-login",
        "feature/a.b",
        "user@host",
        "r1_2",
        "über",
    ];
    for name in valid {
        assert!(valid_branch(name), "{name:?}");
    }
    let invalid = [
        "", "HEAD", "@", "-x", "a..b", "a/", "/a", "a//b", ".a", "a/.b", "a.lock", "a.lock/b",
        "a.", "a b", "a~1", "a^", "a:b", "a?", "a*", "a[b", "a\\b", "a@{1}", "a\tb", "a\u{7f}",
    ];
    for name in invalid {
        assert!(!valid_branch(name), "{name:?}");
    }
}
