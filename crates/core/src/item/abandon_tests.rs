use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, PullState, commit};
use crate::feedback::{Event, EventKind};
use crate::github::GithubRepo;
use crate::highlight::{HighlightStore, Highlights};
use crate::item::{Submit, submit};
use crate::project::{Backend, Project, ProjectName, ProjectStore};

/// `app#1` on `login` at c2, and `app#2` on `copy` at c2 too, so the two share
/// every blob pair. c2 changes `README.md` and adds `src/login.rs`.
fn two_items() -> (FakeStore, FakeGit) {
    let mut store = FakeStore::default();
    store
        .insert_project(&Project {
            name: ProjectName::parse("app").unwrap(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
        })
        .unwrap();
    let git = FakeGit::default()
        .commit(&commit(1), None, "Initial", &["README.md"])
        .commit(
            &commit(2),
            Some(&commit(1)),
            "Add login",
            &["src/login.rs", "README.md"],
        )
        .commit(&commit(3), Some(&commit(2)), "More", &["src/more.rs"])
        .upstream("/src/app", "main", &commit(1));
    for branch in ["login", "copy"] {
        git.push("app", branch, &commit(2));
        let request = Submit {
            project: "app".into(),
            branch: branch.into(),
            head: commit(2),
            title: None,
            worktree: None,
            wake: None,
            guide: None,
        };
        submit(&mut store, &git, &FakeGithub::default(), request).unwrap();
    }
    (store, git)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn abandoning(item: &str, source: Source, reason: Option<&str>) -> Abandon {
    Abandon {
        id: id(item),
        source,
        reason: reason.map(Into::into),
    }
}

fn events(store: &FakeStore, item: &str) -> Vec<Event> {
    store.events(&id(item), 0).unwrap()
}

#[test]
fn closes_the_item_and_tells_its_agent_who_and_why() {
    let (mut store, git) = two_items();
    let reason = Some("  Superseded by app#2.\n");
    let item = abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#1", Source::Operator, reason),
    )
    .unwrap();
    assert_eq!(item.state, ItemState::Abandoned);
    assert_eq!(store.item(&id("app#1")).unwrap().unwrap(), item);
    let [event] = &events(&store, "app#1")[..] else {
        panic!("{:?}", events(&store, "app#1"));
    };
    assert_eq!(
        event.kind,
        EventKind::Notice(Notice {
            revision: 1,
            source: Source::Operator,
            kind: NoticeKind::Abandoned {
                reason: Some("Superseded by app#2.".into()),
            },
        })
    );
    assert!(events(&store, "app#2").is_empty());
}

#[test]
fn a_blank_reason_is_none() {
    let (mut store, git) = two_items();
    abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#1", Source::Agent, Some(" \n")),
    )
    .unwrap();
    let event = &events(&store, "app#1")[0];
    assert_eq!(event.source(), Source::Agent);
    assert_eq!(
        event.kind,
        EventKind::Notice(Notice {
            revision: 1,
            source: Source::Agent,
            kind: NoticeKind::Abandoned { reason: None },
        })
    );
}

#[test]
fn drops_its_pins_and_the_diffs_and_spans_no_open_item_uses() {
    let (mut store, git) = two_items();
    git.push("app", "login", &commit(3));
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(3),
        title: None,
        worktree: None,
        wake: None,
        guide: None,
    };
    submit(&mut store, &git, &FakeGithub::default(), request).unwrap();
    assert_eq!(store.diffs().len(), 3);
    let (login, more) = (format!("{}\n", commit(2)), format!("{}\n", commit(3)));
    for blob in [&login, &more] {
        store
            .insert_highlights(blob, "rs", "1", &Highlights::default())
            .unwrap();
    }
    assert_eq!(
        git.pins().keys().collect::<Vec<_>>(),
        [
            "refs/mergegate/items/1/r1",
            "refs/mergegate/items/1/r2",
            "refs/mergegate/items/2/r1",
        ]
    );

    abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#1", Source::Agent, None),
    )
    .unwrap();
    // app#2 still has README.md and src/login.rs; only r2's src/more.rs goes.
    assert_eq!(store.diffs().len(), 2, "{:?}", store.diffs().keys());
    assert!(!store.diffs().contains_key(&(None, Some(more))));
    assert_eq!(store.highlighted(), [(login, "rs".into(), "1".into())]);
    assert_eq!(
        git.pins().keys().collect::<Vec<_>>(),
        ["refs/mergegate/items/2/r1"]
    );

    abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#2", Source::Agent, None),
    )
    .unwrap();
    assert!(store.diffs().is_empty());
    assert!(store.highlighted().is_empty());
    assert!(git.pins().is_empty());
}

#[test]
fn closes_even_when_dropping_its_pins_fails() {
    let (mut store, git) = two_items();
    let git = git.failing_unpin();
    let item = abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#1", Source::Agent, None),
    )
    .unwrap();
    assert_eq!(item.state, ItemState::Abandoned);
    assert_eq!(git.pins().len(), 2);
}

#[test]
fn abandons_a_held_merge() {
    let (mut store, git) = two_items();
    store.set_state(&id("app#1"), ItemState::Merging);
    let item = abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#1", Source::Operator, None),
    )
    .unwrap();
    assert_eq!(item.state, ItemState::Abandoned);
}

#[test]
fn refuses_a_closed_item_and_an_unknown_one() {
    let (mut store, git) = two_items();
    abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#1", Source::Agent, None),
    )
    .unwrap();
    let err = abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#1", Source::Agent, None),
    )
    .unwrap_err();
    assert!(matches!(err, ItemError::Closed(_)), "{err:?}");
    assert_eq!(events(&store, "app#1").len(), 1);

    store.set_state(&id("app#2"), ItemState::Merged);
    let err = abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#2", Source::Agent, None),
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "app#2 is closed");

    let err = abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        abandoning("app#9", Source::Agent, None),
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "no item app#9");
}

#[test]
fn closes_a_github_items_pull_request_and_deletes_its_branch_first() {
    let mut store = FakeStore::default();
    let acme = GithubRepo::parse("acme/app").unwrap();
    store
        .insert_project(&Project {
            name: ProjectName::parse("app").unwrap(),
            backend: Backend::Github,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: Some(acme.clone()),
            bots: Vec::new(),
        })
        .unwrap();
    let git = FakeGit::default()
        .commit(&commit(1), None, "Initial", &["README.md"])
        .commit(&commit(2), Some(&commit(1)), "Add login", &["src/login.rs"])
        .upstream("https://github.com/acme/app.git", "main", &commit(1));
    git.push("app", "login", &commit(2));
    let github = FakeGithub::default();
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(2),
        title: None,
        worktree: None,
        wake: None,
        guide: None,
    };
    submit(&mut store, &git, &github, request).unwrap();
    // The item still closes on the repository it was opened on.
    store
        .update_project(&Project {
            github: GithubRepo::parse("me/app"),
            ..store.projects().unwrap()[0].clone()
        })
        .unwrap();

    let logged_out = FakeGithub::default().logged_out();
    let err = abandon(
        &mut store,
        &git,
        &logged_out,
        abandoning("app#1", Source::Agent, None),
    )
    .unwrap_err();
    assert!(matches!(err, ItemError::Github(_)), "{err:?}");
    assert!(store.item(&id("app#1")).unwrap().unwrap().state.is_open());

    abandon(
        &mut store,
        &git,
        &github,
        abandoning("app#1", Source::Agent, None),
    )
    .unwrap();
    assert_eq!(github.pulls()[0].state, PullState::Closed);
    assert_eq!(github.deleted(), [(acme, "login".to_owned())]);
}
