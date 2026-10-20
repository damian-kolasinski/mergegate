use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, PullState, commit};
use crate::github::GithubRepo;
use crate::item::{On, Submit, submit};
use crate::project::{ProjectName, ProjectStore};

const GITHUB_APP: &str = "https://github.com/acme/app.git";

/// `app` on GitHub as `acme/app`, with `app#1` open on `login` at c2 as
/// pull request 1.
fn opened() -> (Project, Item, FakeGit, FakeGithub) {
    let (project, mut items, git, github) = submitted(&[("login", 2, None)]);
    (project, items.remove(0), git, github)
}

/// `app` on GitHub as `acme/app`, with an item submitted for each `(branch,
/// head, on)`, in order. c3 is on login's c2.
fn submitted(branches: &[(&str, u8, Option<&str>)]) -> (Project, Vec<Item>, FakeGit, FakeGithub) {
    let mut store = FakeStore::default();
    let project = Project {
        name: ProjectName::parse("app").unwrap(),
        backend: Backend::Github,
        target: "main".into(),
        upstream: "/src/app".into(),
        github: Some(GithubRepo::parse("acme/app").unwrap()),
        bots: Vec::new(),
    };
    store.insert_project(&project).unwrap();
    let git = FakeGit::default()
        .commit(&commit(1), None, "Initial", &["README.md"])
        .commit(&commit(2), Some(&commit(1)), "Add login", &["src/login.rs"])
        .commit(
            &commit(3),
            Some(&commit(2)),
            "Add profile",
            &["src/profile.rs"],
        )
        .upstream(GITHUB_APP, "main", &commit(1));
    let github = FakeGithub::default();
    let mut items = Vec::new();
    for &(branch, head, on) in branches {
        git.push("app", branch, &commit(head));
        let request = Submit {
            project: "app".into(),
            branch: branch.into(),
            head: commit(head),
            title: None,
            worktree: None,
            wake: None,
            device: None,
            guide: None,
            priority: None,
            note: None,
            on: on.map(|on| On::parse(on, &project.name).unwrap()),
        };
        items.push(submit(&mut store, &git, &github, request).unwrap().item);
    }
    (project, items, git, github)
}

fn merged_state(github: &FakeGithub) -> PullState {
    github.pulls()[0].state.clone()
}

#[test]
fn squash_merges_the_pull_request_at_the_approved_head() {
    let (project, item, git, github) = opened();
    let backend = GithubBackend::new(&git, &github);
    assert_eq!(backend.kind(), Backend::Github);
    let message = "Add login (app#1)\n\nSigns users in.\n\nWith a password.";
    let landing = backend.land(&project, &item, message).unwrap();
    assert_eq!(landing, Landing::Merged { commit: commit(8) });
    // GitHub takes the subject as the commit's title, the rest as its body.
    assert_eq!(
        merged_state(&github),
        PullState::Merged {
            head: commit(2),
            title: "Add login (app#1)".into(),
            message: "Signs users in.\n\nWith a password.".into()
        }
    );
    // The branch goes once nothing stacked on the item needs it.
    assert!(github.deleted().is_empty());
    backend.tidy(&project, &item);
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

#[test]
fn a_stacked_pull_request_merges_into_the_target_once_its_parent_has() {
    let stack = [("login", 2, None), ("profile", 3, Some("1"))];
    let (project, items, git, github) = submitted(&stack);
    let [login, profile] = &items[..] else {
        unreachable!()
    };
    let targets = |github: &FakeGithub| {
        let pulls = github.pulls();
        pulls.into_iter().map(|p| p.target).collect::<Vec<_>>()
    };
    assert_eq!(targets(&github), ["main", "login"]);
    let backend = GithubBackend::new(&git, &github);
    backend.land(&project, login, "Add login (app#1)").unwrap();
    backend.restack(&project, profile).unwrap();
    assert_eq!(targets(&github), ["main", "main"]);

    // Landed with its pull request still on the parent's branch, it's
    // pointed at the target first: GitHub merges into whatever it's based on.
    let (project, items, git, github) = submitted(&stack);
    let backend = GithubBackend::new(&git, &github);
    backend
        .land(&project, &items[1], "Add profile (app#2)")
        .unwrap();
    let profile = &github.pulls()[1];
    assert_eq!(profile.target, "main");
    assert!(matches!(profile.state, PullState::Merged { .. }));
}
