use super::*;
use crate::external::{ExternalStore, NewExternal};
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit, submit_login};
use crate::feedback::{Event, EventKind};
use crate::github::{GithubRepo, PullCommentKind};
use crate::item::{On, Submit, submit};
use crate::project::{Backend, Project, ProjectName, ProjectStore};

/// `app#1` on `login` at c2, pull request 1 of `acme/app`.
fn on_github() -> (FakeStore, FakeGit) {
    let mut store = FakeStore::default();
    store
        .insert_project(&Project {
            name: ProjectName::parse("app").unwrap(),
            backend: Backend::Github,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: GithubRepo::parse("acme/app"),
            bots: Vec::new(),
        })
        .unwrap();
    let git = FakeGit::default()
        .commit(&commit(1), None, "Initial", &["README.md"])
        .commit(&commit(2), Some(&commit(1)), "Add login", &["src/login.rs"])
        .upstream("https://github.com/acme/app.git", "main", &commit(1));
    git.push("app", "login", &commit(2));
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(2),
        title: None,
        worktree: None,
        wake: None,
        guide: None,
        priority: None,
        note: None,
        on: None,
    };
    submit(&mut store, &git, &FakeGithub::default(), request).unwrap();
    (store, git)
}

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn events(store: &FakeStore) -> Vec<Event> {
    store.events(&id(), 0).unwrap()
}

/// Merged at r1's head.
fn merged() -> PullStatus {
    PullStatus::Merged {
        commit: commit(8),
        head: commit(2),
        base: "main".into(),
    }
}

#[test]
fn a_pull_request_merged_on_github_merges_the_item() {
    let (mut store, git) = on_github();
    let item = follow_pull(&mut store, &git, &id(), 1, merged())
        .unwrap()
        .unwrap();
    assert_eq!(item.state, ItemState::Merged);
    assert_eq!(store.item(&id()).unwrap().unwrap(), item);
    assert_eq!(
        events(&store)[0].kind,
        EventKind::Notice(Notice {
            revision: 1,
            source: Source::External,
            kind: NoticeKind::Merged { commit: commit(8) },
        })
    );
    // A merged item keeps its newest revision, which its diff is read from.
    assert_eq!(
        git.pins().keys().collect::<Vec<_>>(),
        ["refs/mergegate/items/1/r1"]
    );
}

#[test]
fn a_pull_request_closed_on_github_abandons_the_item() {
    let (mut store, git) = on_github();
    let item = follow_pull(&mut store, &git, &id(), 1, PullStatus::Closed)
        .unwrap()
        .unwrap();
    assert_eq!(item.state, ItemState::Abandoned);
    assert_eq!(
        events(&store)[0].kind,
        EventKind::Notice(Notice {
            revision: 1,
            source: Source::External,
            kind: NoticeKind::Abandoned {
                reason: Some(CLOSED_ON_GITHUB.into()),
            },
        })
    );
    assert!(git.pins().is_empty());
}

/// `app#1`, with profile at c3 stacked on it.
fn stacked() -> (FakeStore, FakeGit, Item) {
    let (mut store, git) = on_github();
    let git = git.commit(
        &commit(3),
        Some(&commit(2)),
        "Add profile",
        &["src/profile.rs"],
    );
    git.push("app", "profile", &commit(3));
    let request = Submit {
        project: "app".into(),
        branch: "profile".into(),
        head: commit(3),
        title: None,
        worktree: None,
        wake: None,
        guide: None,
        priority: None,
        note: None,
        on: Some(On::Item(id())),
    };
    let child = submit(&mut store, &git, &FakeGithub::default(), request)
        .unwrap()
        .item;
    (store, git, child)
}

#[test]
fn a_parent_closed_on_github_hands_its_child_back() {
    let (mut store, git, child) = stacked();
    follow_pull(&mut store, &git, &id(), 1, PullStatus::Closed).unwrap();
    let child = store.item(&child.id).unwrap().unwrap();
    assert_eq!(child.state, ItemState::ChangesRequested);
    let kinds: Vec<_> = store
        .events(&child.id, 0)
        .unwrap()
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert_eq!(
        kinds,
        [EventKind::Notice(Notice {
            revision: 1,
            source: Source::Operator,
            kind: NoticeKind::ParentAbandoned {
                parent: id(),
                upstream: commit(2),
            },
        })]
    );
}

#[test]
fn a_child_merged_on_github_into_its_parents_branch_did_not_land() {
    let (mut store, git, child) = stacked();
    let ItemBackend::Github { pull, .. } = &child.backend else {
        panic!("{child:?}");
    };
    let into_login = PullStatus::Merged {
        commit: commit(8),
        head: commit(3),
        base: "login".into(),
    };
    let closed = follow_pull(&mut store, &git, &child.id, pull.number, into_login)
        .unwrap()
        .unwrap();
    assert_eq!(closed.state, ItemState::Abandoned);
    let events = store.events(&child.id, 0).unwrap();
    let EventKind::Notice(Notice {
        kind: NoticeKind::Abandoned {
            reason: Some(reason),
        },
        ..
    }) = &events[0].kind
    else {
        panic!("{events:?}");
    };
    assert!(reason.contains("into login, not main"), "{reason}");
    assert!(store.item(&id()).unwrap().unwrap().state.is_open());
}

#[test]
fn a_held_merge_follows_its_pull_request_too() {
    let (mut store, git) = on_github();
    store.set_state(&id(), ItemState::Merging);
    let item = follow_pull(&mut store, &git, &id(), 1, merged()).unwrap();
    assert_eq!(item.unwrap().state, ItemState::Merged);
    // The operator approved it in the hub, wherever it landed.
    assert_eq!(events(&store)[0].source(), Source::Operator);
}

#[test]
fn a_merge_of_an_earlier_head_abandons_the_revision_that_did_not_land() {
    let (mut store, git) = on_github();
    let earlier = PullStatus::Merged {
        commit: commit(8),
        head: commit(1),
        base: "main".into(),
    };
    let item = follow_pull(&mut store, &git, &id(), 1, earlier)
        .unwrap()
        .unwrap();
    assert_eq!(item.state, ItemState::Abandoned);
    let EventKind::Notice(Notice {
        source: Source::External,
        kind: NoticeKind::Abandoned {
            reason: Some(reason),
        },
        ..
    }) = &events(&store)[0].kind
    else {
        panic!("{:?}", events(&store));
    };
    assert!(reason.contains(&commit(1)), "{reason}");
    assert!(reason.contains(&commit(2)), "{reason}");
}

#[test]
fn held_comments_reach_the_agent_before_the_close() {
    let (mut store, git) = on_github();
    let comment = NewExternal {
        item: id(),
        revision: 1,
        anchor: None,
        body: "Closing: wrong approach.".into(),
        kind: PullCommentKind::IssueComment,
        github_id: 1,
        login: "alice".into(),
        bot: false,
        url: String::new(),
        path: None,
        dated: 0,
    };
    store.insert_external(comment, 0).unwrap();
    follow_pull(&mut store, &git, &id(), 1, PullStatus::Closed).unwrap();
    let events = events(&store);
    assert!(
        matches!(&events[0].kind, EventKind::ExternalComments(_)),
        "{events:?}"
    );
    assert!(matches!(
        &events[1].kind,
        EventKind::Notice(Notice {
            kind: NoticeKind::Abandoned { .. },
            ..
        })
    ));
    assert_eq!(store.held(&id()).unwrap(), None);
}

#[test]
fn an_open_pull_request_leaves_the_item_open() {
    let (mut store, git) = on_github();
    let item = follow_pull(&mut store, &git, &id(), 1, PullStatus::Open).unwrap();
    assert_eq!(item, None);
    assert!(store.item(&id()).unwrap().unwrap().state.is_open());
    assert!(events(&store).is_empty());
}

#[test]
fn an_item_the_hub_closed_meanwhile_is_left_as_it_is() {
    let (mut store, git) = on_github();
    follow_pull(&mut store, &git, &id(), 1, PullStatus::Closed).unwrap();
    // Read before the close, the pull request was merged.
    let item = follow_pull(&mut store, &git, &id(), 1, merged()).unwrap();
    assert_eq!(item, None);
    assert_eq!(
        store.item(&id()).unwrap().unwrap().state,
        ItemState::Abandoned
    );
    assert_eq!(events(&store).len(), 1);
}

#[test]
fn another_pull_request_and_a_local_item_are_left_alone() {
    let (mut store, git) = on_github();
    assert_eq!(
        follow_pull(&mut store, &git, &id(), 2, merged()).unwrap(),
        None
    );

    let (mut store, git) = crate::fake::rebased();
    let local = submit_login(&mut store, &git, 2);
    assert_eq!(
        follow_pull(&mut store, &git, &local.id, 1, merged()).unwrap(),
        None
    );
    assert!(store.item(&local.id).unwrap().unwrap().state.is_open());
}

#[test]
fn an_unknown_item_is_an_error() {
    let (mut store, git) = on_github();
    let missing = ItemId::parse("app#9").unwrap();
    let err = follow_pull(&mut store, &git, &missing, 1, merged()).unwrap_err();
    assert_eq!(err.to_string(), "no item app#9");
}
