use std::path::PathBuf;

use super::*;

fn repo(branches: &[&str], head: Option<&str>) -> Repo {
    let mut branches: Vec<String> = branches.iter().map(|b| (*b).into()).collect();
    branches.sort();
    Repo {
        root: PathBuf::from("/repo"),
        branches,
        head: head.map(Into::into),
        remotes: Vec::new(),
    }
}

/// The options a decision offers and its default.
fn decided(target: Target) -> (Vec<String>, String) {
    match target {
        Target::Decide(d) => (
            d.options.into_iter().map(|o| o.value).collect(),
            d.default.unwrap(),
        ),
        Target::Chosen(b) => panic!("chose {b} without asking"),
    }
}

#[test]
fn picks_the_target_without_asking_when_it_is_clear() {
    let cases = [
        (repo(&["main", "feature"], Some("feature")), None, "main"),
        (repo(&["master", "dev"], Some("dev")), None, "master"),
        (repo(&["trunk"], None), None, "trunk"),
        (repo(&["develop"], Some("develop")), None, "develop"),
        (
            repo(&["main", "release"], Some("main")),
            Some("release"),
            "release",
        ),
    ];
    for (repo, requested, expected) in cases {
        assert_eq!(
            choose(&repo, requested).unwrap(),
            Target::Chosen(expected.into()),
            "{repo:?} {requested:?}"
        );
    }
}

#[test]
fn asks_between_conventional_branches_defaulting_to_the_checked_out_one() {
    let target = choose(&repo(&["main", "master", "x"], Some("master")), None).unwrap();
    assert_eq!(
        decided(target),
        (vec!["main".into(), "master".into()], "master".into())
    );
}

#[test]
fn asks_between_all_branches_when_none_is_conventional() {
    let target = choose(&repo(&["dev", "prod"], Some("prod")), None).unwrap();
    let Target::Decide(decision) = target else {
        panic!("expected a decision")
    };
    assert_eq!(decision.flag, "--target");
    assert_eq!(decision.default.as_deref(), Some("prod"));
    assert_eq!(
        decision.options[1].description.as_deref(),
        Some("checked out")
    );
    assert_eq!(decision.options[0].description, None);

    // A detached HEAD defaults to the first branch.
    let target = choose(&repo(&["dev", "prod"], None), None).unwrap();
    assert_eq!(decided(target).1, "dev");
}

#[test]
fn caps_the_options_but_keeps_the_checked_out_branch() {
    let names: Vec<String> = (0..20).map(|i| format!("b{i:02}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let (options, default) = decided(choose(&repo(&names, Some("b15")), None).unwrap());
    assert_eq!(options.len(), MAX_OPTIONS);
    assert!(options.contains(&"b15".to_owned()));
    assert_eq!(default, "b15");
}

#[test]
fn a_requested_branch_must_exist() {
    let err = choose(&repo(&["main"], None), Some("nope")).unwrap_err();
    assert!(matches!(err, ProjectError::UnknownBranch { branch, .. } if branch == "nope"));
}
