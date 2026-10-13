use super::*;
use crate::fake::FakeStore;
use crate::item::{FileStat, ItemId, NewItem, NewRevision};
use crate::project::{Backend, ProjectName};

fn add(store: &mut FakeStore, project: &str, branch: &str) -> ItemId {
    let file = |path: &str| FileStat {
        path: path.into(),
        old_path: None,
        additions: Some(3),
        deletions: Some(1),
    };
    let item = NewItem {
        project: ProjectName::parse(project).unwrap(),
        branch: branch.into(),
        title: format!("Work on {branch}"),
        state: ItemState::NeedsReview,
        backend: Backend::Local,
        target: "main".into(),
        revision: NewRevision {
            head: "1".repeat(40),
            base: "0".repeat(40),
            files: vec![file("a.rs"), file("b.png")],
        },
    };
    store.insert_item(item).unwrap().id
}

#[test]
fn groups_open_items_by_who_acts_next_oldest_first() {
    let mut store = FakeStore::default();
    let a = add(&mut store, "app", "a");
    let b = add(&mut store, "web", "b");
    let c = add(&mut store, "app", "c");
    let d = add(&mut store, "app", "d");
    let e = add(&mut store, "app", "e");
    store.set_state(&b, ItemState::ChangesRequested);
    store.set_state(&d, ItemState::Merged);
    store.set_state(&e, ItemState::Merging);

    let inbox = inbox(&store).unwrap();
    let ids = |group: &[InboxItem]| group.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&inbox.needs_you), [a.to_string(), c.to_string()]);
    assert_eq!(ids(&inbox.with_agents), [b.to_string()]);
    assert_eq!(ids(&inbox.merging), [e.to_string()]);

    let first = &inbox.needs_you[0];
    assert_eq!(first.title, "Work on a");
    assert_eq!(first.branch, "a");
    assert_eq!(
        (
            first.revision,
            first.files,
            first.additions,
            first.deletions
        ),
        (1, 2, 6, 2)
    );
    assert_eq!(first.updated_at, first.created_at);
}
