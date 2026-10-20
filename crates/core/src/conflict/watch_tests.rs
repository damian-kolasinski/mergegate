use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit};
use crate::github::Pull;
use crate::item::{ItemGit, NewItem, NewRevision};
use crate::merge::{Blocked, blocked};
use crate::project::{Backend, Project};

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

/// `app` targets main, at c1 upstream.
fn setup() -> (FakeStore, FakeGit) {
    let mut store = FakeStore::default();
    store
        .insert_project(&Project {
            name: app(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
        })
        .unwrap();
    let git = FakeGit::default().upstream("/src/app", "main", &commit(1));
    (store, git)
}

fn add(store: &mut FakeStore, branch: &str, head: u8, backend: ItemBackend) -> ItemId {
    add_on(store, branch, head, backend, None)
}

fn add_on(
    store: &mut FakeStore,
    branch: &str,
    head: u8,
    backend: ItemBackend,
    parent: Option<&ItemId>,
) -> ItemId {
    let item = NewItem {
        project: app(),
        branch: branch.into(),
        title: format!("Work on {branch}"),
        state: ItemState::NeedsReview,
        backend,
        target: "main".into(),
        revision: NewRevision {
            head: commit(head),
            base: commit(1),
            files: Vec::new(),
            guide: None,
            interdiffs: Vec::new(),
            viewed: Vec::new(),
            parent: parent.cloned(),
        },
        priority: None,
        note: None,
    };
    store.insert_item(item).unwrap().id
}

/// A whole pass, the store held throughout.
fn watch(
    store: &mut FakeStore,
    git: &FakeGit,
    github: &FakeGithub,
    project: &ProjectName,
    seen: &mut Seen,
) -> Result<Vec<ItemId>, ConflictError> {
    let Some(watch) = Watch::read(store, project)? else {
        return Ok(Vec::new());
    };
    let mut merges = watch.try_merges(git, github, seen);
    if let Some(e) = merges.errors.pop() {
        return Err(e);
    }
    let back = merges.hand_back(store, git)?;
    *seen = merges.seen;
    Ok(back)
}

fn state(store: &FakeStore, id: &ItemId) -> ItemState {
    store.item(id).unwrap().unwrap().state
}

#[test]
fn a_target_that_moves_into_a_conflict_hands_the_item_back_once() {
    let (mut store, git) = setup();
    let id = add(&mut store, "login", 2, ItemBackend::Local);
    let github = FakeGithub::default();
    let mut seen = Seen::default();

    let back = watch(&mut store, &git, &github, &app(), &mut seen).unwrap();
    assert!(back.is_empty());
    assert_eq!(state(&store, &id), ItemState::NeedsReview);
    assert_eq!(git.merged(), [(commit(1), commit(2))]);

    // Nothing moved: only the fetch.
    watch(&mut store, &git, &github, &app(), &mut seen).unwrap();
    assert_eq!(git.merged().len(), 1);
    assert_eq!(git.fetched().len(), 2);

    git.push_upstream("/src/app", "main", &commit(5));
    git.conflict_between(&commit(5), &commit(2), &["src/login.rs"]);
    let back = watch(&mut store, &git, &github, &app(), &mut seen).unwrap();
    assert_eq!(back, std::slice::from_ref(&id));
    // Staging's target moves with it, for the agent to rebase onto.
    assert_eq!(git.branch_head(&app(), "main").unwrap(), Some(commit(5)));
    let item = store.item(&id).unwrap().unwrap();
    assert_eq!(item.state, ItemState::ChangesRequested);
    assert_eq!(
        blocked(&store, &item, false).unwrap(),
        Some(Blocked::Conflict {
            target_commit: commit(5),
            paths: vec!["src/login.rs".into()],
        })
    );

    // Once per revision, even as the target moves on and a restarted hub
    // forgets what it tried.
    git.push_upstream("/src/app", "main", &commit(6));
    git.conflict_between(&commit(6), &commit(2), &["src/login.rs"]);
    let back = watch(&mut store, &git, &github, &app(), &mut Seen::default()).unwrap();
    assert!(back.is_empty());
    assert_eq!(store.events(&id, 0).unwrap().len(), 1);
    // Staging still follows, so the rebase lands on where the target is.
    assert_eq!(git.branch_head(&app(), "main").unwrap(), Some(commit(6)));
}

#[test]
fn a_stacked_item_is_left_to_its_parent_until_it_lands() {
    let (mut store, git) = setup();
    let login = add(&mut store, "login", 2, ItemBackend::Local);
    let profile = add_on(&mut store, "profile", 3, ItemBackend::Local, Some(&login));
    let github = FakeGithub::default();
    git.push_upstream("/src/app", "main", &commit(5));
    git.conflict_between(&commit(5), &commit(2), &["src/login.rs"]);
    git.conflict_between(&commit(5), &commit(3), &["src/login.rs"]);

    let back = watch(&mut store, &git, &github, &app(), &mut Seen::default()).unwrap();
    assert_eq!(back, std::slice::from_ref(&login));
    assert_eq!(git.merged(), [(commit(2), commit(5))]);
    assert_eq!(state(&store, &profile), ItemState::NeedsReview);

    // Once its parent closed, its agent was told how to rebase: a trial
    // from where its head forked would replay the parent's commits.
    store.set_state(&login, ItemState::Abandoned);
    let back = watch(&mut store, &git, &github, &app(), &mut Seen::default()).unwrap();
    assert!(back.is_empty(), "{back:?}");
    assert_eq!(state(&store, &profile), ItemState::NeedsReview);
}

#[test]
fn a_new_revision_is_tried_again() {
    let (mut store, git) = setup();
    let id = add(&mut store, "login", 2, ItemBackend::Local);
    let github = FakeGithub::default();
    let mut seen = Seen::default();
    git.conflict_between(&commit(1), &commit(2), &["a.rs"]);
    watch(&mut store, &git, &github, &app(), &mut seen).unwrap();
    assert_eq!(state(&store, &id), ItemState::ChangesRequested);

    store
        .add_revision(
            &id,
            "Work on login",
            ItemState::NeedsReview,
            NewRevision {
                head: commit(3),
                base: commit(1),
                files: Vec::new(),
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
                parent: None,
            },
        )
        .unwrap();
    let back = watch(&mut store, &git, &github, &app(), &mut seen).unwrap();
    assert!(back.is_empty());
    assert_eq!(state(&store, &id), ItemState::NeedsReview);
    assert_eq!(git.merged().last(), Some(&(commit(1), commit(3))));
}

#[test]
fn leaves_items_in_merging_to_their_merge() {
    let (mut store, git) = setup();
    let id = add(&mut store, "login", 2, ItemBackend::Local);
    store.set_state(&id, ItemState::Merging);
    git.conflict_between(&commit(1), &commit(2), &["a.rs"]);
    let back = watch(
        &mut store,
        &git,
        &FakeGithub::default(),
        &app(),
        &mut Seen::default(),
    )
    .unwrap();
    assert!(back.is_empty());
    assert!(git.merged().is_empty());
    assert!(git.fetched().is_empty());
    assert_eq!(state(&store, &id), ItemState::Merging);
}

#[test]
fn fetches_once_per_place_the_targets_come_from() {
    let (mut store, git) = setup();
    let git = git.upstream("https://github.com/acme/app.git", "main", &commit(7));
    let repo = GithubRepo::parse("acme/app").unwrap();
    let pull = |number| Pull {
        number,
        url: format!("https://github.com/acme/app/pull/{number}"),
    };
    add(&mut store, "a", 2, ItemBackend::Local);
    add(&mut store, "b", 3, ItemBackend::Local);
    let on_github = add(
        &mut store,
        "c",
        4,
        ItemBackend::Github {
            repo: repo.clone(),
            pull: pull(1),
        },
    );
    add(
        &mut store,
        "d",
        5,
        ItemBackend::Github {
            repo,
            pull: pull(2),
        },
    );
    git.conflict_between(&commit(7), &commit(4), &["x.rs"]);

    let back = watch(
        &mut store,
        &git,
        &FakeGithub::default(),
        &app(),
        &mut Seen::default(),
    )
    .unwrap();
    assert_eq!(back, [on_github]);
    assert_eq!(git.fetched().len(), 2);
    let mut merged = git.merged();
    merged.sort();
    assert_eq!(
        merged,
        [
            (commit(1), commit(2)),
            (commit(1), commit(3)),
            (commit(4), commit(7)),
            (commit(5), commit(7)),
        ]
    );
}

#[test]
fn a_target_that_fails_leaves_the_others_tried() {
    let (mut store, git) = setup();
    add(
        &mut store,
        "c",
        4,
        ItemBackend::Github {
            repo: GithubRepo::parse("acme/app").unwrap(),
            pull: Pull {
                number: 1,
                url: "https://github.com/acme/app/pull/1".into(),
            },
        },
    );
    let local = add(&mut store, "d", 5, ItemBackend::Local);
    git.conflict_between(&commit(1), &commit(5), &["d.rs"]);

    let watch = Watch::read(&store, &app()).unwrap().unwrap();
    let merges = watch.try_merges(&git, &FakeGithub::default().logged_out(), &Seen::default());
    assert!(matches!(merges.errors[..], [ConflictError::Github(_)]));
    assert_eq!(merges.hand_back(&mut store, &git).unwrap(), [local]);
}

#[test]
fn an_item_that_moved_on_while_merging_is_left_alone() {
    let (mut store, git) = setup();
    let revised = add(&mut store, "a", 2, ItemBackend::Local);
    let landing = add(&mut store, "b", 3, ItemBackend::Local);
    git.conflict_between(&commit(1), &commit(2), &["a.rs"]);
    git.conflict_between(&commit(1), &commit(3), &["b.rs"]);
    let watch = Watch::read(&store, &app()).unwrap().unwrap();
    let merges = watch.try_merges(&git, &FakeGithub::default(), &Seen::default());
    assert_eq!(merges.found.len(), 2);

    // Without the store held, the agent submits again and the operator
    // approves the other.
    store
        .add_revision(
            &revised,
            "Work on a",
            ItemState::NeedsReview,
            NewRevision {
                head: commit(4),
                base: commit(1),
                files: Vec::new(),
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
                parent: None,
            },
        )
        .unwrap();
    store.set_state(&landing, ItemState::Merging);
    assert!(merges.hand_back(&mut store, &git).unwrap().is_empty());
    assert_eq!(state(&store, &revised), ItemState::NeedsReview);
    assert_eq!(state(&store, &landing), ItemState::Merging);
    assert!(store.events(&revised, 0).unwrap().is_empty());
}
