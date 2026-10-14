use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, PullState, commit};
use crate::github::GithubRepo;
use crate::item::{Submit, submit};
use crate::project::{ProjectName, ProjectStore};

const GITHUB_APP: &str = "https://github.com/acme/app.git";

/// `app` on GitHub as `acme/app`, with `app#1` open on `login` at c2 as
/// pull request 1.
fn opened() -> (Project, Item, FakeGit, FakeGithub) {
    let mut store = FakeStore::default();
    let project = Project {
        name: ProjectName::parse("app").unwrap(),
        backend: Backend::Github,
        target: "main".into(),
        upstream: "/src/app".into(),
        github: Some(GithubRepo::parse("acme/app").unwrap()),
    };
    store.insert_project(&project).unwrap();
    let git = FakeGit::default()
        .commit(&commit(1), None, "Initial", &["README.md"])
        .commit(&commit(2), Some(&commit(1)), "Add login", &["src/login.rs"])
        .upstream(GITHUB_APP, "main", &commit(1));
    let github = FakeGithub::default();
    git.push("app", "login", &commit(2));
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(2),
        title: None,
        worktree: None,
        wake: None,
    };
    let item = submit(&mut store, &git, &github, request).unwrap().item;
    (project, item, git, github)
}

fn merged_state(github: &FakeGithub) -> PullState {
    github.pulls()[0].state.clone()
}

#[test]
fn squash_merges_the_pull_request_at_the_approved_head() {
    let (project, item, git, github) = opened();
    let backend = GithubBackend::new(&git, &github);
    assert_eq!(backend.kind(), Backend::Github);
    let landing = backend.land(&project, &item, "Add login (app#1)").unwrap();
    assert_eq!(landing, Landing::Merged { commit: commit(8) });
    assert_eq!(
        merged_state(&github),
        PullState::Merged {
            head: commit(2),
            title: "Add login (app#1)".into()
        }
    );
    let repo = GithubRepo::parse("acme/app").unwrap();
    assert_eq!(github.deleted(), [(repo, "login".to_owned())]);

    // Landing again, as a retry after the hub failed to record it, finds
    // the merge GitHub already made.
    let again = backend.land(&project, &item, "Add login (app#1)").unwrap();
    assert_eq!(again, Landing::Merged { commit: commit(8) });
}

#[test]
fn a_conflict_found_in_staging_is_never_merged() {
    let (project, item, git, github) = opened();
    let git = git.conflicting(&["README.md"]);
    let landing = GithubBackend::new(&git, &github)
        .land(&project, &item, "Add login (app#1)")
        .unwrap();
    assert_eq!(
        landing,
        Landing::Conflict {
            target_commit: commit(1),
            paths: vec!["README.md".into()]
        }
    );
    assert_eq!(merged_state(&github), PullState::Open);
    assert!(github.deleted().is_empty());

    // Still a conflict when GitHub can't say whether it merged the branch.
    let unknown = FakeGithub::default();
    let landing = GithubBackend::new(&git, &unknown)
        .land(&project, &item, "Add login (app#1)")
        .unwrap();
    assert!(matches!(landing, Landing::Conflict { .. }), "{landing:?}");
}

#[test]
fn a_merge_the_hub_never_recorded_lands_though_the_target_moved_on() {
    let (project, item, git, github) = opened();
    let backend = GithubBackend::new(&git, &github);
    backend.land(&project, &item, "Add login (app#1)").unwrap();

    // Later commits on the target conflict with the branch it merged.
    let git = git.conflicting(&["src/login.rs"]);
    let landing = GithubBackend::new(&git, &github)
        .land(&project, &item, "Add login (app#1)")
        .unwrap();
    assert_eq!(landing, Landing::Merged { commit: commit(8) });
}

#[test]
fn holds_what_github_refuses_or_cant_be_asked() {
    let (project, item, git, github) = opened();
    github.refuse("Required status check \"ci\" is failing");
    let landing = GithubBackend::new(&git, &github)
        .land(&project, &item, "")
        .unwrap();
    assert_eq!(
        landing,
        Landing::Held {
            reason: "GitHub refused: Required status check \"ci\" is failing".into()
        }
    );

    let logged_out = FakeGithub::default().logged_out();
    let landing = GithubBackend::new(&git, &logged_out)
        .land(&project, &item, "")
        .unwrap();
    let Landing::Held { reason } = landing else {
        panic!("{landing:?}");
    };
    assert!(
        reason.starts_with("the hub can't sign in to GitHub"),
        "{reason}"
    );

    let local = Item {
        backend: ItemBackend::Local,
        ..item
    };
    let landing = GithubBackend::new(&git, &github)
        .land(&project, &local, "")
        .unwrap();
    assert_eq!(
        landing,
        Landing::Held {
            reason: "app#1 has no pull request on GitHub".into()
        }
    );
    assert_eq!(merged_state(&github), PullState::Open);
}
