use super::*;

#[test]
fn parses_an_identity_as_git_var_prints_it() {
    let ident = Ident::parse("Ada Lovelace <ada@example.com> 1700000000 +0200").unwrap();
    assert_eq!(ident.name, "Ada Lovelace");
    assert_eq!(ident.email, "ada@example.com");
    assert_eq!(ident.date, "1700000000 +0200");
}

#[test]
fn an_identity_without_an_email_is_none() {
    assert!(Ident::parse("Ada 1700000000 +0200").is_none());
}

#[test]
fn a_dirty_checkout_says_to_commit_or_stash() {
    let reason = held(
        Path::new("/Users/ada/code/app"),
        Some(Path::new("/Users/ada")),
        "main",
        "Working directory has unstaged changes",
    );
    let (advice, details) = reason.split_once("\n\n").unwrap();
    assert_eq!(
        advice,
        "~/code/app has main checked out with uncommitted changes. \
         Commit or stash them, then retry the merge."
    );
    assert_eq!(
        details,
        "git: ~/code/app refused main: Working directory has unstaged changes"
    );
}

#[test]
fn any_other_refusal_keeps_the_full_path_outside_home() {
    let reason = held(
        Path::new("/srv/app"),
        Some(Path::new("/Users/ada")),
        "main",
        "hook declined",
    );
    assert!(
        reason.starts_with("/srv/app turned down the merge into main."),
        "{reason}"
    );
    assert!(
        reason.ends_with("git: /srv/app refused main: hook declined"),
        "{reason}"
    );
}
