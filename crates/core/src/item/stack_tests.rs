use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit};
use crate::feedback::EventKind;
use crate::item::{Submit, Submitted, submit};
use crate::project::{Backend, Project, ProjectStore};

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

/// `app` targets main at c1. login is c2 on it, then c5 once amended;
/// profile is c3 on login's c2, then c6 on its c5; settings is c8, also on
/// c2. c7 is on main alone.
fn setup() -> (FakeStore, FakeGit) {
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
        .commit(
            &commit(5),
            Some(&commit(1)),
            "Add login",
            &["src/login.rs", "src/session.rs"],
        )
        .commit(
            &commit(6),
            Some(&commit(5)),
            "Add profile",
            &["src/profile.rs"],
        )
        .commit(
            &commit(7),
            Some(&commit(1)),
            "Add search",
            &["src/search.rs"],
        )
        .commit(
            &commit(8),
            Some(&commit(2)),
            "Add settings",
            &["src/settings.rs"],
        )
        .upstream("/src/app", "main", &commit(1));
    (store, git)
}

fn send(
    store: &mut FakeStore,
    git: &FakeGit,
    branch: &str,
    head: u8,
    on: Option<On>,
) -> Result<Submitted, ItemError> {
    git.push("app", branch, &commit(head));
    submit(
        store,
        git,
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
}

/// login as app#1, and profile stacked on it as app#2.
fn stacked() -> (FakeStore, FakeGit) {
    let (mut store, git) = setup();
    send(&mut store, &git, "login", 2, None).unwrap();
    send(&mut store, &git, "profile", 3, Some(On::Item(id("app#1")))).unwrap();
    (store, git)
}

fn item(store: &FakeStore, item: &str) -> Item {
    store.item(&id(item)).unwrap().unwrap()
}

#[test]
fn on_names_an_item_by_id_or_number_or_none() {
    let app = ProjectName::parse("app").unwrap();
    assert_eq!(On::parse("web#4", &app).unwrap(), On::Item(id("web#4")));
    assert_eq!(On::parse(" 4 ", &app).unwrap(), On::Item(id("app#4")));
    assert_eq!(On::parse("none", &app).unwrap(), On::Target);
    assert!(On::parse("four", &app).is_err());
    assert!(On::parse("app#", &app).is_err());
}

#[test]
fn a_stacked_item_is_reviewed_by_its_own_diff_against_its_parent() {
    let (store, _git) = stacked();
    let child = item(&store, "app#2");
    assert_eq!(child.parent, Some(id("app#1")));
    assert_eq!(child.target, "main");
    assert_eq!(child.revision.base, commit(2));
    assert_eq!(child.revision.stat.files, 1);
    assert!(!behind(&store, &child).unwrap());

    let ids = |stack: Stack| {
        stack
            .items
            .into_iter()
            .map(|i| i.id.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(stack(&store, &child).unwrap()), ["app#1", "app#2"]);
    assert_eq!(
        ids(stack(&store, &item(&store, "app#1")).unwrap()),
        ["app#1", "app#2"]
    );
}

#[test]
fn a_parent_takes_one_child() {
    let (mut store, git) = stacked();
    let e = send(&mut store, &git, "settings", 8, Some(On::Item(id("app#1")))).unwrap_err();
    assert!(
        matches!(&e, ItemError::Stack(StackError::HasChild { child, .. }) if *child == id("app#2")),
        "{e}"
    );
    assert_eq!(
        e.to_string(),
        "app#2 is already stacked on app#1; stack on app#2 instead"
    );
    // On the child is where it goes, so long as it's built on it.
    let e = send(&mut store, &git, "settings", 8, Some(On::Item(id("app#2")))).unwrap_err();
    assert!(
        matches!(e, ItemError::Stack(StackError::NotOnParent { .. })),
        "{e}"
    );
}

#[test]
fn refuses_what_it_cant_stack_on() {
    let (mut store, git) = stacked();
    let on = |item| Some(On::Item(id(item)));
    let refused = |store: &mut FakeStore, branch, head, parent| {
        send(store, &git, branch, head, on(parent)).unwrap_err()
    };
    assert!(matches!(
        refused(&mut store, "search", 7, "web#1"),
        ItemError::Stack(StackError::OtherProject { .. })
    ));
    assert!(matches!(
        refused(&mut store, "search", 7, "app#9"),
        ItemError::NoItem(_)
    ));
    assert!(matches!(
        refused(&mut store, "search", 7, "app#1"),
        ItemError::Stack(StackError::HasChild { .. })
    ));
    assert!(matches!(
        refused(&mut store, "search", 7, "app#2"),
        ItemError::Stack(StackError::NotOnParent { .. })
    ));
    assert!(matches!(
        refused(&mut store, "profile", 3, "app#2"),
        ItemError::Stack(StackError::OnItself(_))
    ));
    assert!(matches!(
        refused(&mut store, "login", 2, "app#2"),
        ItemError::Stack(StackError::Cycle { .. })
    ));
    store.set_state(&id("app#2"), ItemState::Merged);
    assert!(matches!(
        refused(&mut store, "search", 7, "app#2"),
        ItemError::Stack(StackError::ParentClosed(_))
    ));
    assert_eq!(store.items().len(), 2);
}

#[test]
fn says_which_branch_to_build_on() {
    let (mut store, git) = stacked();
    let e = send(&mut store, &git, "search", 7, Some(On::Item(id("app#2")))).unwrap_err();
    assert_eq!(
        e.to_string(),
        "search has none of app#2's revisions; build it on profile, or submit with --on none"
    );
}

#[test]
fn a_new_parent_revision_tells_the_child_to_rebase_onto_it() {
    let (mut store, git) = stacked();
    store.set_state(&id("app#2"), ItemState::Merging);
    send(&mut store, &git, "login", 5, None).unwrap();

    let child = item(&store, "app#2");
    assert_eq!(
        child.state,
        ItemState::Merging,
        "its own diff didn't change"
    );
    assert!(behind(&store, &child).unwrap());
    let events = store.events(&child.id, 0).unwrap();
    let kinds: Vec<_> = events.into_iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [EventKind::Notice(Notice {
            revision: 1,
            source: Source::Operator,
            kind: NoticeKind::ParentUpdated {
                parent: id("app#1"),
                merged: false,
                onto: commit(5),
                upstream: commit(2),
            },
        })]
    );

    // Rebased, it stays on its parent without saying so again.
    let rebased = send(&mut store, &git, "profile", 6, None).unwrap().item;
    assert_eq!(rebased.parent, Some(id("app#1")));
    assert_eq!(rebased.revision.base, commit(5));
    assert_eq!(rebased.revision.stat.files, 1);
    assert!(!behind(&store, &rebased).unwrap());
}

#[test]
fn on_none_moves_the_item_onto_its_target_at_the_same_head() {
    let (mut store, git) = stacked();
    let submitted = send(&mut store, &git, "profile", 3, Some(On::Target)).unwrap();
    assert!(submitted.new_revision);
    let item = submitted.item;
    assert_eq!(item.parent, None);
    assert_eq!(item.revision.number, 2);
    assert_eq!(item.revision.base, commit(1));
    assert_eq!(item.revision.stat.files, 2);
    assert_eq!(store.child(&id("app#1")).unwrap(), None);
}

#[test]
fn a_closed_parent_leaves_the_next_revision_on_the_target() {
    let (mut store, git) = stacked();
    store.set_state(&id("app#1"), ItemState::Merged);
    assert!(behind(&store, &item(&store, "app#2")).unwrap());
    let item = send(&mut store, &git, "profile", 6, None).unwrap().item;
    assert_eq!(item.parent, None);
}

/// app#1 merged as `commit`, which main is now at.
fn merged_as(store: &mut FakeStore, git: &FakeGit, commit: &str) {
    let notice = Notice {
        revision: 1,
        source: Source::Operator,
        kind: NoticeKind::Merged {
            commit: commit.into(),
        },
    };
    store
        .notify(&id("app#1"), ItemState::Merged, notice)
        .unwrap();
    git.push_upstream("/src/app", "main", commit);
}

#[test]
fn a_child_still_on_its_squashed_parent_is_refused() {
    let (mut store, git) = stacked();
    let git = squashed(git);
    merged_as(&mut store, &git, &commit(9));
    // Still on c2, which the target has only squashed, its diff would hold
    // its parent's again.
    let e = send(&mut store, &git, "profile", 8, None).unwrap_err();
    assert!(
        matches!(&e, ItemError::Stack(StackError::ParentMerged { upstream, .. }) if *upstream == commit(2)),
        "{e}"
    );
    let item = send(&mut store, &git, "profile", 10, None).unwrap().item;
    assert_eq!(item.parent, None);
    assert_eq!(item.revision.base, commit(9));
    assert_eq!(item.revision.stat.files, 1);
}

#[test]
fn a_child_on_a_rewritten_parent_revision_is_refused_too() {
    let (mut store, git) = stacked();
    send(&mut store, &git, "login", 5, None).unwrap();
    let git = squashed(git);
    merged_as(&mut store, &git, &commit(9));
    // Rebased onto login's c5, which doesn't hold the c2 it was built on.
    let e = send(&mut store, &git, "profile", 6, None).unwrap_err();
    assert!(
        matches!(&e, ItemError::Stack(StackError::ParentMerged { upstream, .. }) if *upstream == commit(5)),
        "{e}"
    );
}

/// login squashed onto main as c9, and profile alone on it as c10.
fn squashed(git: FakeGit) -> FakeGit {
    git.commit(&commit(9), Some(&commit(1)), "Add login", &["src/login.rs"])
        .commit(
            &commit(10),
            Some(&commit(9)),
            "Add profile",
            &["src/profile.rs"],
        )
}

#[test]
fn a_child_on_a_parent_the_target_has_whole_goes_as_it_is() {
    let (mut store, git) = stacked();
    merged_as(&mut store, &git, &commit(2));
    let item = send(&mut store, &git, "profile", 8, None).unwrap().item;
    assert_eq!(item.revision.base, commit(2));
}
