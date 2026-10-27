use std::cell::RefCell;

use crate::fake::{FakeBackend, FakeGit, FakeGithub, FakeStore, commit};
use crate::feedback::{EventKind, FeedbackStore, Notice, NoticeKind, Source};
use crate::git::GitError;
use crate::item::{Abandon, Item, ItemId, ItemState, ItemStore, On, Submit, abandon, submit};
use crate::merge::{
    Approve, Blocked, Landing, MergeBackend, MergeError, approve, blocked, merged_elsewhere, retry,
};
use crate::project::{Backend, Project, ProjectName, ProjectStore};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

/// login as app#1 at c2 on main's c1, and profile at c3 stacked on it as
/// app#2.
fn stacked() -> (FakeStore, FakeGit) {
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
        .commit(&commit(2), Some(&commit(1)), "Add login", &["src/login.rs"])
        .commit(
            &commit(3),
            Some(&commit(2)),
            "Add profile",
            &["src/profile.rs"],
        )
        .upstream("/src/app", "main", &commit(1));
    for (branch, head, on) in [
        ("login", 2, None),
        ("profile", 3, Some(On::Item(id("app#1")))),
    ] {
        git.push("app", branch, &commit(head));
        submit(
            &mut store,
            &git,
            &FakeGithub::default(),
            Submit {
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
                on,
            },
        )
        .unwrap();
    }
    (store, git)
}

fn approving(item: &str) -> Approve {
    Approve {
        id: id(item),
        revision: 1,
        summary: String::new(),
    }
}

fn item(store: &FakeStore, item: &str) -> Item {
    store.item(&id(item)).unwrap().unwrap()
}

fn notices(store: &FakeStore, item: &str) -> Vec<NoticeKind> {
    let events = store.events(&id(item), 0).unwrap();
    events
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::Notice(notice) => Some(notice.kind),
            _ => None,
        })
        .collect()
}

fn landed(backend: &FakeBackend) -> Vec<String> {
    backend.landed().into_iter().map(|(id, _)| id).collect()
}

#[test]
fn an_approved_child_waits_for_its_parent_then_lands_right_after_it() {
    let (mut store, git) = stacked();
    let backend = FakeBackend::new(Backend::Local);
    let waiting = approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#2"),
    )
    .unwrap();
    assert_eq!(
        waiting.landing,
        Landing::Waiting {
            parent: id("app#1")
        }
    );
    assert_eq!(waiting.item.state, ItemState::Merging);
    assert!(waiting.stacked.is_empty());
    assert!(
        landed(&backend).is_empty(),
        "nothing is tried before its parent lands"
    );
    assert_eq!(
        blocked(&store, &waiting.item, false).unwrap(),
        Some(Blocked::Waiting {
            parent: id("app#1")
        })
    );
    // A retry waits the same way.
    let retried = retry(&RefCell::new(&mut store), &git, &[&backend], &id("app#2")).unwrap();
    assert_eq!(
        retried.landing,
        Landing::Waiting {
            parent: id("app#1")
        }
    );

    let merge = approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#1"),
    )
    .unwrap();
    assert_eq!(merge.item.state, ItemState::Merged);
    let [child] = &merge.stacked[..] else {
        panic!("{:?}", merge.stacked);
    };
    assert_eq!(
        (child.id.to_string(), child.state),
        ("app#2".into(), ItemState::Merged)
    );
    assert_eq!(landed(&backend), ["app#1", "app#2"]);
    // The child's review moves off the parent's branch before it goes.
    assert_eq!(
        backend.stacking(),
        ["restack app#2", "tidy app#1", "tidy app#2"]
    );
    assert_eq!(
        notices(&store, "app#2"),
        [NoticeKind::Merged { commit: commit(9) }]
    );
}

#[test]
fn a_whole_approved_stack_lands_once_its_bottom_does() {
    let (mut store, git) = stacked();
    let git = git.commit(
        &commit(4),
        Some(&commit(3)),
        "Add settings",
        &["src/settings.rs"],
    );
    git.push("app", "settings", &commit(4));
    let on_profile = Submit {
        project: "app".into(),
        branch: "settings".into(),
        head: commit(4),
        title: None,
        worktree: None,
        wake: None,
        device: None,
        guide: None,
        priority: None,
        note: None,
        on: Some(On::Item(id("app#2"))),
    };
    submit(&mut store, &git, &FakeGithub::default(), on_profile).unwrap();
    let backend = FakeBackend::new(Backend::Local);
    for waiting in ["app#3", "app#2"] {
        approve(
            &RefCell::new(&mut store),
            &git,
            &[&backend],
            approving(waiting),
        )
        .unwrap();
    }

    let merge = approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#1"),
    )
    .unwrap();
    let stacked: Vec<_> = merge
        .stacked
        .iter()
        .map(|item| (item.id.to_string(), item.state))
        .collect();
    assert_eq!(
        stacked,
        [
            ("app#2".into(), ItemState::Merged),
            ("app#3".into(), ItemState::Merged)
        ]
    );
    assert_eq!(landed(&backend), ["app#1", "app#2", "app#3"]);
}

#[test]
fn a_child_whose_diff_doesnt_apply_waits_in_merging_for_its_agent_to_rebase() {
    let (mut store, git) = stacked();
    let backend = FakeBackend::new(Backend::Local)
        .then(Ok(Landing::Merged { commit: commit(9) }))
        .then(Ok(Landing::Conflict {
            target_commit: commit(9),
            paths: vec!["src/login.rs".into()],
        }));
    approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#2"),
    )
    .unwrap();
    let merge = approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#1"),
    )
    .unwrap();

    let child = item(&store, "app#2");
    assert_eq!(merge.stacked, std::slice::from_ref(&child));
    assert_eq!(child.state, ItemState::Merging, "it keeps its approval");
    assert_eq!(
        notices(&store, "app#2"),
        [NoticeKind::ParentUpdated {
            parent: id("app#1"),
            merged: true,
            onto: commit(9),
            upstream: commit(2),
        }]
    );
    assert_eq!(
        blocked(&store, &child, false).unwrap(),
        Some(Blocked::Behind {
            parent: id("app#1")
        })
    );
}

#[test]
fn a_child_not_yet_approved_hears_its_parent_merged() {
    let (mut store, git) = stacked();
    let backend = FakeBackend::new(Backend::Local);
    let merge = approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#1"),
    )
    .unwrap();
    assert_eq!(landed(&backend), ["app#1"]);
    let child = item(&store, "app#2");
    assert_eq!(merge.stacked, std::slice::from_ref(&child));
    assert_eq!(child.state, ItemState::NeedsReview);
    assert_eq!(
        notices(&store, "app#2"),
        [NoticeKind::ParentUpdated {
            parent: id("app#1"),
            merged: true,
            onto: commit(9),
            upstream: commit(2),
        }]
    );
}

#[test]
fn a_child_whose_review_cant_move_keeps_its_parents_branch() {
    let (mut store, git) = stacked();
    let backend = FakeBackend::new(Backend::Local).failing_restack();
    approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#2"),
    )
    .unwrap();
    approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#1"),
    )
    .unwrap();
    assert_eq!(landed(&backend), ["app#1"]);
    assert_eq!(backend.stacking(), ["restack app#2"]);
    let child = item(&store, "app#2");
    assert_eq!(child.state, ItemState::Merging);
    assert_eq!(
        notices(&store, "app#2"),
        [NoticeKind::MergeHeld {
            reason: "app#1 merged, but the restack failed".into(),
        }]
    );

    // Landing it moves its review off, so the parent's branch can go.
    let working = FakeBackend::new(Backend::Local);
    retry(&RefCell::new(&mut store), &git, &[&working], &id("app#2")).unwrap();
    assert_eq!(working.stacking(), ["tidy app#2", "tidy app#1"]);
}

#[test]
fn an_abandoned_parent_hands_its_child_back_and_bars_its_approval() {
    let (mut store, git) = stacked();
    let backend = FakeBackend::new(Backend::Local);
    abandon(
        &mut store,
        &git,
        &FakeGithub::default(),
        Abandon {
            id: id("app#1"),
            source: Source::Operator,
            reason: None,
            leave_github: false,
        },
    )
    .unwrap();
    let child = item(&store, "app#2");
    assert_eq!(child.state, ItemState::ChangesRequested);
    let events = store.events(&child.id, 0).unwrap();
    assert_eq!(
        events.into_iter().map(|e| e.kind).collect::<Vec<_>>(),
        [EventKind::Notice(Notice {
            revision: 1,
            source: Source::Operator,
            kind: NoticeKind::ParentAbandoned {
                parent: id("app#1"),
                upstream: commit(2),
            },
        })]
    );
    let e = approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#2"),
    )
    .unwrap_err();
    assert!(matches!(e, MergeError::ParentAbandoned { .. }), "{e}");
    assert_eq!(
        e.to_string(),
        "app#1, which app#2 is stacked on, was abandoned; app#2 needs a new base first"
    );
}

#[test]
fn approving_a_child_left_on_its_merged_parents_branch_lets_that_branch_go() {
    let (mut store, git) = stacked();
    let backend = FakeBackend::new(Backend::Local).failing_restack();
    approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#1"),
    )
    .unwrap();
    assert_eq!(backend.stacking(), ["restack app#2"]);

    let working = FakeBackend::new(Backend::Local);
    approve(
        &RefCell::new(&mut store),
        &git,
        &[&working],
        approving("app#2"),
    )
    .unwrap();
    assert_eq!(working.stacking(), ["tidy app#2", "tidy app#1"]);
}

#[test]
fn a_parent_merged_elsewhere_moves_its_child_as_the_hubs_own_merge_would() {
    let (mut store, git) = stacked();
    let backend = FakeBackend::new(Backend::Local);
    approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#2"),
    )
    .unwrap();
    let notice = Notice {
        revision: 1,
        source: Source::External,
        kind: NoticeKind::Merged { commit: commit(8) },
    };
    store
        .notify(&id("app#1"), ItemState::Merged, notice)
        .unwrap();

    let moved = merged_elsewhere(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        &id("app#1"),
        &commit(8),
    )
    .unwrap();
    let moved: Vec<_> = moved
        .into_iter()
        .map(|i| (i.id.to_string(), i.state))
        .collect();
    assert_eq!(moved, [("app#2".to_owned(), ItemState::Merged)]);
    assert_eq!(landed(&backend), ["app#2"]);
    assert_eq!(
        backend.stacking(),
        ["restack app#2", "tidy app#1", "tidy app#2"]
    );
}

#[test]
fn a_failed_parent_landing_leaves_the_child_waiting() {
    let (mut store, git) = stacked();
    let backend =
        FakeBackend::new(Backend::Local).then(Err(GitError::Failed("staging is gone".into())));
    approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#2"),
    )
    .unwrap();
    let merge = approve(
        &RefCell::new(&mut store),
        &git,
        &[&backend],
        approving("app#1"),
    )
    .unwrap();
    assert!(matches!(merge.landing, Landing::Held { .. }));
    assert!(merge.stacked.is_empty());
    let child = item(&store, "app#2");
    assert_eq!(
        blocked(&store, &child, false).unwrap(),
        Some(Blocked::Waiting {
            parent: id("app#1")
        })
    );
}

/// Hands the child back to its agent, as the conflict watch would, while
/// its parent's merge restacks it with the store free.
struct HandingBack<'a, 's> {
    backend: FakeBackend,
    store: &'a RefCell<&'s mut FakeStore>,
}

impl MergeBackend for HandingBack<'_, '_> {
    fn kind(&self) -> Backend {
        self.backend.kind()
    }

    fn land(&self, project: &Project, item: &Item, message: &str) -> Result<Landing, GitError> {
        self.backend.land(project, item, message)
    }

    fn restack(&self, project: &Project, child: &Item) -> Result<(), GitError> {
        let notice = Notice {
            revision: child.revision.number,
            source: Source::Operator,
            kind: NoticeKind::MergeConflict {
                target_commit: commit(1),
                paths: vec!["src/profile.rs".into()],
            },
        };
        self.store
            .borrow_mut()
            .notify(&child.id, ItemState::ChangesRequested, notice)
            .unwrap();
        self.backend.restack(project, child)
    }
}

#[test]
fn a_child_handed_back_while_its_parent_merges_stays_handed_back() {
    let (mut store, git) = stacked();
    let shared = RefCell::new(&mut store);
    let backend = HandingBack {
        backend: FakeBackend::new(Backend::Local),
        store: &shared,
    };
    let merge = approve(&shared, &git, &[&backend], approving("app#1")).unwrap();
    assert_eq!(merge.stacked[0].state, ItemState::ChangesRequested);
    assert_eq!(item(&store, "app#2").state, ItemState::ChangesRequested);
}
