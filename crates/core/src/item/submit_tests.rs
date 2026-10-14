use super::*;
use crate::diff::Content;
use crate::fake::{FakeGit, FakeStore, commit};
use crate::item::{DiffStat, ItemId};
use crate::project::{Backend, Project, ProjectError, ProjectName};

fn project(name: &str, upstream: &str, target: &str) -> Project {
    Project {
        name: ProjectName::parse(name).unwrap(),
        backend: Backend::Local,
        target: target.into(),
        upstream: upstream.into(),
    }
}

/// `app` targets main at c1. c2 and c3 build a branch on it; c4 moves main
/// on; c9 shares no history with any of them.
fn setup() -> (FakeStore, FakeGit) {
    let mut store = FakeStore::default();
    store
        .insert_project(&project("app", "/src/app", "main"))
        .unwrap();
    store
        .insert_project(&project("web", "/src/web", "main"))
        .unwrap();
    let git = FakeGit::default()
        .commit(&commit(1), None, "Initial", &["README.md"])
        .commit(&commit(2), Some(&commit(1)), "Add login", &["src/login.rs"])
        .commit(
            &commit(3),
            Some(&commit(2)),
            "Fix typo",
            &["src/login.rs", "README.md"],
        )
        .commit(
            &commit(4),
            Some(&commit(1)),
            "Upstream change",
            &["CHANGELOG"],
        )
        .commit(&commit(9), None, "Unrelated", &["x"])
        .upstream("/src/app", "main", &commit(1))
        .upstream("/src/app", "release", &commit(1))
        .upstream("/src/web", "main", &commit(1));
    (store, git)
}

fn request(project: &str, branch: &str, head: u8) -> Submit {
    Submit {
        project: project.into(),
        branch: branch.into(),
        head: commit(head),
        title: None,
        worktree: None,
        wake: None,
    }
}

#[test]
fn opens_an_item_against_the_merge_base() {
    let (mut store, git) = setup();
    git.push("app", "login", &commit(3));
    let submitted = submit(&mut store, &git, request("app", "login", 3)).unwrap();
    assert!(submitted.new_revision);
    let item = submitted.item;
    assert_eq!(item.id.to_string(), "app#1");
    assert_eq!(item.title, "Add login");
    assert_eq!(item.state, ItemState::NeedsReview);
    assert_eq!(
        (item.backend, item.target.as_str()),
        (Backend::Local, "main")
    );
    assert_eq!(item.revision.number, 1);
    assert_eq!(item.revision.base, commit(1));
    assert_eq!(
        item.revision.stat,
        DiffStat {
            files: 2,
            additions: 3,
            deletions: 0
        }
    );
    assert_eq!(git.fetched(), [("app".into(), "main".into())]);
    assert_eq!(git.pins()["refs/mergegate/items/1/r1"], commit(3));
    assert_eq!(store.items(), [item]);
}

#[test]
fn a_new_head_adds_a_revision_and_returns_the_item_for_review() {
    let (mut store, mut git) = setup();
    git.push("app", "login", &commit(2));
    let first = submit(&mut store, &git, request("app", "login", 2))
        .unwrap()
        .item;
    store.set_state(&first.id, ItemState::ChangesRequested);
    // The target moved on meanwhile; the base is still the merge-base.
    git = git.upstream("/src/app", "main", &commit(4));
    git.push("app", "login", &commit(3));
    let second = submit(&mut store, &git, request("app", "login", 3)).unwrap();
    assert!(second.new_revision);
    let item = second.item;
    assert_eq!((item.id.clone(), item.revision.number), (first.id, 2));
    assert_eq!(item.state, ItemState::NeedsReview);
    assert_eq!(item.title, "Add login");
    assert_eq!(item.revision.base, commit(1));
    assert_eq!(item.created_at, first.created_at);
    assert_ne!(item.revision.submitted_at, first.revision.submitted_at);
    assert_eq!(git.pins()["refs/mergegate/items/1/r2"], commit(3));
    assert_eq!(store.items().len(), 1);
}

#[test]
fn caches_each_files_diff_and_reuses_it_in_later_revisions() {
    let (mut store, git) = setup();
    let git = git.commit(&commit(5), Some(&commit(3)), "Changelog", &["CHANGELOG"]);
    git.push("app", "login", &commit(3));
    submit(&mut store, &git, request("app", "login", 3)).unwrap();
    let login = (None, Some(format!("{}\n{}\n", commit(2), commit(3))));
    let Content::Text(hunks) = &store.diffs()[&login].content else {
        panic!("not text");
    };
    assert_eq!(
        (hunks[0].old_start, hunks[0].new_start, hunks[0].lines.len()),
        (0, 1, 2)
    );
    let read = git.blobs_read().len();

    git.push("app", "login", &commit(5));
    let item = submit(&mut store, &git, request("app", "login", 5))
        .unwrap()
        .item;
    assert_eq!(git.blobs_read()[read..], [format!("{}\n", commit(5))]);
    assert_eq!(
        item.revision.stat,
        DiffStat {
            files: 3,
            additions: 4,
            deletions: 0
        }
    );
}

#[test]
fn the_same_head_again_returns_the_current_revision() {
    let (mut store, git) = setup();
    git.push("app", "login", &commit(3));
    let first = submit(&mut store, &git, request("app", "login", 3)).unwrap();
    let again = submit(&mut store, &git, request("app", "login", 3)).unwrap();
    assert!(!again.new_revision);
    assert_eq!(again.item, first.item);
    assert_eq!(git.fetched().len(), 1);
}

#[test]
fn records_the_agents_worktree_and_wake_even_at_the_same_head() {
    use crate::agent::{Agent, AgentStore};
    let (mut store, git) = setup();
    git.push("app", "login", &commit(3));
    let first = Submit {
        worktree: Some("/work/login".into()),
        ..request("app", "login", 3)
    };
    let id = submit(&mut store, &git, first).unwrap().item.id;
    let agent = |worktree: &str, wake: Option<&str>| Agent {
        worktree: Some(worktree.into()),
        wake: wake.map(Into::into),
    };
    assert_eq!(store.agent(&id).unwrap(), agent("/work/login", None));
    // Waking needs no new commits: the agent submits again with --wake.
    let again = Submit {
        wake: Some("resume {item}".into()),
        ..request("app", "login", 3)
    };
    assert!(!submit(&mut store, &git, again).unwrap().new_revision);
    assert_eq!(
        store.agent(&id).unwrap(),
        agent("/work/login", Some("resume {item}"))
    );
}

#[test]
fn takes_the_title_given_over_the_commit_subject() {
    let (mut store, git) = setup();
    git.push("app", "login", &commit(2));
    let mut titled = request("app", "login", 2);
    titled.title = Some("  Login page ".into());
    let item = submit(&mut store, &git, titled).unwrap().item;
    assert_eq!(item.title, "Login page");

    git.push("app", "login", &commit(3));
    let mut blank = request("app", "login", 3);
    blank.title = Some(" ".into());
    let item = submit(&mut store, &git, blank).unwrap().item;
    assert_eq!(item.title, "Login page");
}

#[test]
fn numbers_items_per_project_and_closed_branches_start_over() {
    let (mut store, git) = setup();
    git.push("app", "a", &commit(2));
    git.push("app", "b", &commit(3));
    git.push("web", "a", &commit(2));
    let ids: Vec<String> = [("app", "a"), ("app", "b"), ("web", "a")]
        .into_iter()
        .map(|(project, branch)| {
            let head = git.branch_head(&ProjectName::parse(project).unwrap(), branch);
            let mut request = request(project, branch, 0);
            request.head = head.unwrap().unwrap();
            submit(&mut store, &git, request)
                .unwrap()
                .item
                .id
                .to_string()
        })
        .collect();
    assert_eq!(ids, ["app#1", "app#2", "web#1"]);

    let first = ItemId::parse("app#1").unwrap();
    store.set_state(&first, ItemState::Merged);
    let item = submit(&mut store, &git, request("app", "a", 2))
        .unwrap()
        .item;
    assert_eq!(
        (item.id.to_string(), item.revision.number),
        ("app#3".into(), 1)
    );
}

#[test]
fn open_items_keep_their_target_when_the_project_changes() {
    let (mut store, git) = setup();
    git.push("app", "login", &commit(2));
    submit(&mut store, &git, request("app", "login", 2)).unwrap();
    store
        .update_project(&project("app", "/src/app", "release"))
        .unwrap();

    git.push("app", "login", &commit(3));
    let item = submit(&mut store, &git, request("app", "login", 3))
        .unwrap()
        .item;
    assert_eq!(item.target, "main");
    git.push("app", "other", &commit(2));
    let item = submit(&mut store, &git, request("app", "other", 2))
        .unwrap()
        .item;
    assert_eq!(item.target, "release");
    assert_eq!(
        git.fetched()
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>(),
        ["main", "main", "release"]
    );
}

#[test]
fn refuses_what_it_cant_submit() {
    let (mut store, git) = setup();
    git.push("app", "on-main", &commit(1));
    git.push("app", "unrelated", &commit(9));
    git.push("app", "login", &commit(2));
    let mut err = |request| submit(&mut store, &git, request).unwrap_err();

    assert!(matches!(
        err(request("nope", "login", 2)),
        ItemError::Project(ProjectError::NotFound(_))
    ));
    assert!(matches!(
        err(request("app", "a..b", 2)),
        ItemError::InvalidBranch(_)
    ));
    let mut short = request("app", "login", 2);
    short.head = "abc123".into();
    assert!(matches!(err(short), ItemError::InvalidCommit(_)));
    assert_eq!(
        err(request("app", "main", 1)).to_string(),
        "main is the target branch; submit work from its own branch"
    );
    assert!(matches!(
        err(request("app", "pushed-nowhere", 2)),
        ItemError::NoBranch(_)
    ));
    assert!(matches!(
        err(request("app", "login", 3)),
        ItemError::NotPushed { found, .. } if found == commit(2)
    ));
    assert_eq!(
        err(request("app", "on-main", 1)).to_string(),
        "on-main has no commits that aren't on main"
    );
    assert!(matches!(
        err(request("app", "unrelated", 9)),
        ItemError::Unrelated { .. }
    ));
    assert!(store.items().is_empty());
    assert!(git.pins().is_empty());
}
