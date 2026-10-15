use api::project::Backend;

use super::*;

fn project(name: &str, upstream: &str, github: Option<&str>) -> Project {
    Project {
        name: name.into(),
        backend: match github {
            Some(_) => Backend::Github,
            None => Backend::Local,
        },
        target: "main".into(),
        upstream: upstream.into(),
        github: github.map(Into::into),
        bots: Vec::new(),
    }
}

fn names(projects: &[Project], remotes: &[&str], local: bool) -> Vec<String> {
    // Named like a clone's: origin, then upstream.
    let remotes: Vec<(String, String)> = ["origin", "upstream"]
        .iter()
        .zip(remotes)
        .map(|(name, url)| ((*name).to_owned(), (*url).to_owned()))
        .collect();
    named(projects, &remotes, local)
}

fn named(projects: &[Project], remotes: &[(String, String)], local: bool) -> Vec<String> {
    matching(projects, remotes, local, |name| {
        format!("http://mac-mini:7800/git/{name}")
    })
    .into_iter()
    .map(|p| p.name.clone())
    .collect()
}

#[test]
fn a_clone_finds_its_project_through_its_remotes() {
    let projects = [
        project("app", "/Users/op/app", Some("acme/App")),
        project("tools", "/Users/op/tools", None),
    ];
    let cases: [(&[&str], bool, &[&str]); 9] = [
        (&["git@github.com:acme/app.git"], false, &["app"]),
        (&["https://github.com/Acme/App"], false, &["app"]),
        (&["http://mac-mini:7800/git/tools"], false, &["tools"]),
        (&["http://mac-mini:7800/git/tools/"], false, &["tools"]),
        (&["/Users/op/tools"], true, &["tools"]),
        // Another machine's path names nothing on the hub's.
        (&["/Users/op/tools"], false, &[]),
        (&["git@github.com:acme/other.git"], false, &[]),
        (&[], false, &[]),
        (
            &[
                "git@github.com:acme/app.git",
                "http://mac-mini:7800/git/tools",
            ],
            false,
            &["app", "tools"],
        ),
    ];
    for (remotes, local, expected) in cases {
        assert_eq!(
            names(&projects, remotes, local),
            expected,
            "{remotes:?} local={local}"
        );
    }
}

#[test]
fn the_mergegate_remote_settles_a_shared_github_repo() {
    let projects = [
        project("app", "/Users/op/app", Some("acme/app")),
        project("fork", "/Users/op/fork", Some("acme/app")),
    ];
    let remote = |name: &str, url: &str| (name.to_owned(), url.to_owned());
    let origin = remote("origin", "git@github.com:acme/app.git");
    assert_eq!(
        named(&projects, std::slice::from_ref(&origin), false),
        ["app", "fork"]
    );
    let inited = [
        origin.clone(),
        remote("mergegate", "http://mac-mini:7800/git/fork"),
    ];
    assert_eq!(named(&projects, &inited, false), ["fork"]);
    // One pointing at no project settles nothing.
    let stale = [origin, remote("mergegate", "http://mac-mini:7800/git/gone")];
    assert_eq!(named(&projects, &stale, false), ["app", "fork"]);
}
