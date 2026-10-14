use super::*;

fn remote(name: &str, url: &str) -> RepoRemote {
    RepoRemote {
        name: name.into(),
        url: url.into(),
    }
}

fn repo(value: &str) -> GithubRepo {
    GithubRepo::parse(value).unwrap()
}

#[test]
fn prefers_origin() {
    let remotes = [
        remote("fork", "git@github.com:me/app.git"),
        remote("origin", "https://github.com/team/app.git"),
    ];
    assert_eq!(detect(&remotes), Detected::One(repo("team/app")));
}

#[test]
fn takes_the_only_github_remote_when_origin_is_elsewhere() {
    let remotes = [
        remote("origin", "git@gitlab.com:team/app.git"),
        remote("gh", "git@github.com:team/app.git"),
        remote("mirror", "https://github.com/team/app"),
    ];
    assert_eq!(detect(&remotes), Detected::One(repo("team/app")));
}

#[test]
fn lists_several_github_repos_without_origin() {
    let remotes = [
        remote("b", "git@github.com:team/app.git"),
        remote("a", "git@github.com:me/app.git"),
    ];
    assert_eq!(
        detect(&remotes),
        Detected::Several(vec![repo("me/app"), repo("team/app")])
    );
}

#[test]
fn finds_none_without_a_github_remote() {
    assert_eq!(detect(&[]), Detected::None);
    assert_eq!(
        detect(&[remote("origin", "/srv/git/app.git")]),
        Detected::None
    );
}
