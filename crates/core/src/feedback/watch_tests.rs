use super::*;
use crate::agent::Agent;
use crate::fake::FakeStore;
use crate::feedback::{Notice, NoticeKind, Source, acknowledge};
use crate::item::{ItemBackend, ItemState, NewItem, NewRevision};
use crate::project::ProjectName;

fn add(store: &mut FakeStore, branch: &str, worktree: &str, wake: Option<&str>) -> ItemId {
    let item = NewItem {
        project: ProjectName::parse("app").unwrap(),
        branch: branch.into(),
        title: format!("Work on {branch}"),
        state: ItemState::NeedsReview,
        backend: ItemBackend::Local,
        target: "main".into(),
        revision: NewRevision {
            head: "1".repeat(40),
            base: "0".repeat(40),
            files: Vec::new(),
            guide: None,
            interdiffs: Vec::new(),
            viewed: Vec::new(),
            parent: None,
        },
        priority: None,
        note: None,
    };
    let id = store.insert_item(item).unwrap().id;
    let agent = Agent {
        worktree: Some(worktree.into()),
        wake: wake.map(Into::into),
        device: None,
    };
    store.set_agent(&id, &agent).unwrap();
    id
}

fn abandon(store: &mut FakeStore, id: &ItemId) -> u64 {
    let notice = Notice {
        revision: 1,
        source: Source::Operator,
        kind: NoticeKind::Abandoned { reason: None },
    };
    store
        .notify(id, ItemState::Abandoned, notice)
        .unwrap()
        .cursor
}

fn at(worktree: &str) -> Watch {
    Watch {
        device: None,
        worktree: Some(worktree.into()),
        items: Vec::new(),
    }
}

fn ids(watching: &Watching) -> Vec<String> {
    watching
        .items
        .iter()
        .map(|w| w.item.id.to_string())
        .collect()
}

#[test]
fn watches_only_what_the_session_owns_without_a_wake() {
    let mut store = FakeStore::default();
    let own = add(&mut store, "own", "/Users/op/a", None);
    add(&mut store, "other", "/Users/op/b", None);
    add(&mut store, "woken", "/Users/op/a", Some("resume"));
    let elsewhere = add(&mut store, "elsewhere", "/Users/op/c", None);

    assert_eq!(
        ids(&watch(&store, &at("/Users/op/a"), None).unwrap()),
        [own.to_string()]
    );

    // The session submitted one item from another worktree, and names one
    // it says came from a worktree it didn't.
    let mut session = at("/Users/op/a");
    session.items = vec![
        (elsewhere.clone(), "/Users/op/c".into()),
        (ItemId::parse("app#2").unwrap(), "/Users/op/a".into()),
    ];
    assert_eq!(
        ids(&watch(&store, &session, None).unwrap()),
        [own.to_string(), elsewhere.to_string()]
    );

    // Another device's hook in the same path watches nothing of this one's.
    let mut remote = at("/Users/op/a");
    remote.device = Some("laptop".into());
    assert!(watch(&store, &remote, None).unwrap().items.is_empty());
}

#[test]
fn names_the_unread_event_worth_waking_for() {
    let mut store = FakeStore::default();
    let own = add(&mut store, "own", "/Users/op/a", None);
    let mut session = at("/Users/op/a");
    session.items = vec![(ItemId::parse("app#9").unwrap(), "/Users/op/a".into())];

    let quiet = watch(&store, &session, None).unwrap();
    assert_eq!(quiet.items.len(), 1);
    assert_eq!(quiet.items[0].cursor, None);

    let abandoned = abandon(&mut store, &own);
    let read = watch(&store, &session, None).unwrap();
    assert_eq!(read.items[0].cursor, Some(abandoned));
    assert_eq!(read.items[0].item.state, ItemState::Abandoned);
    assert_eq!(read.cursor, abandoned);

    // Read by its agent, it no longer wakes the session.
    acknowledge(&mut store, &own, abandoned, None).unwrap();
    assert_eq!(watch(&store, &session, None).unwrap().items[0].cursor, None);
}

#[test]
fn changes_only_with_a_watched_items_events_after_the_cursor() {
    let mut store = FakeStore::default();
    let own = add(&mut store, "own", "/Users/op/a", None);
    let other = add(&mut store, "other", "/Users/op/b", None);
    let session = at("/Users/op/a");

    let start = watch(&store, &session, None).unwrap();
    assert!(!start.changed);

    let theirs = abandon(&mut store, &other);
    let read = watch(&store, &session, Some(start.cursor)).unwrap();
    assert!(!read.changed, "another session's item moved");
    assert_eq!(read.cursor, theirs);

    abandon(&mut store, &own);
    assert!(watch(&store, &session, Some(read.cursor)).unwrap().changed);
    assert!(watch(&store, &session, Some(start.cursor)).unwrap().changed);
}
