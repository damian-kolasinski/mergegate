use super::*;
use crate::conflict::{ConflictStore, Pair};
use crate::fake::FakeStore;
use crate::item::{ItemBackend, ItemId, NewItem, NewRevision, Priority};
use crate::project::ProjectName;
use crate::review::{Author, NewComment, NewReply, ReplyStatus, ReviewStore, Side};

fn add(store: &mut FakeStore, project: &str, branch: &str) -> ItemId {
    add_on(store, project, branch, None)
}

fn add_on(store: &mut FakeStore, project: &str, branch: &str, parent: Option<&ItemId>) -> ItemId {
    let item = NewItem {
        project: ProjectName::parse(project).unwrap(),
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
            parent: parent.cloned(),
        },
        priority: None,
        note: None,
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
    let ids = |group: &[Item]| group.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&inbox.needs_you), [a, c]);
    assert_eq!(ids(&inbox.with_agents), [b]);
    assert_eq!(ids(&inbox.merging), [e]);
}

#[test]
fn a_stack_reads_bottom_first_and_ranks_by_what_it_holds_up() {
    let mut store = FakeStore::default();
    let search = add(&mut store, "app", "search");
    let login = add(&mut store, "app", "login");
    let profile = add_on(&mut store, "app", "profile", Some(&login));
    let avatar = add_on(&mut store, "app", "avatar", Some(&profile));
    let theme = add(&mut store, "app", "theme");
    let settings = add_on(&mut store, "app", "settings", Some(&theme));
    store.set_state(&theme, ItemState::Merging);

    let inbox = inbox(&store).unwrap();
    let ids = |group: &[Item]| group.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    // login holds up two, so comes before search, older but holding none.
    assert_eq!(
        ids(&inbox.needs_you),
        [login, profile, avatar, search, settings]
    );
    assert_eq!(ids(&inbox.merging), [theme]);
}

#[test]
fn orders_each_group_by_priority_then_age() {
    let mut store = FakeStore::default();
    let ids: Vec<_> = ["a", "b", "c", "d", "e"]
        .into_iter()
        .map(|branch| add(&mut store, "app", branch))
        .collect();
    let priorities = [
        None,
        Some(Priority::P2),
        Some(Priority::P0),
        None,
        Some(Priority::P2),
    ];
    for (id, priority) in ids.iter().zip(priorities) {
        store.set_priority(id, priority, None).unwrap();
    }
    store.set_state(&ids[3], ItemState::ChangesRequested);

    let inbox = inbox(&store).unwrap();
    let ids_of = |group: &[Item]| group.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    let [a, b, c, d, e] = ids.try_into().unwrap();
    assert_eq!(ids_of(&inbox.needs_you), [c, b, e, a]);
    assert_eq!(ids_of(&inbox.with_agents), [d]);
}

#[test]
fn breaks_priority_ties_by_how_many_items_each_conflicts_with() {
    let mut store = FakeStore::default();
    let ids: Vec<_> = ["a", "b", "c", "d", "e"]
        .into_iter()
        .map(|branch| add(&mut store, "app", branch))
        .collect();
    let [a, b, c, d, e] = ids.try_into().unwrap();
    store.set_priority(&a, Some(Priority::P1), None).unwrap();
    let pair = |item: &ItemId, other: &ItemId| Pair {
        item: item.clone(),
        item_head: "1".repeat(40),
        other: other.clone(),
        other_head: "1".repeat(40),
        conflicts: true,
    };
    let project = ProjectName::parse("app").unwrap();
    store
        .set_pairs(&project, &[pair(&b, &d), pair(&b, &e), pair(&d, &e)])
        .unwrap();
    store.set_state(&b, ItemState::Merging);

    let inbox = inbox(&store).unwrap();
    let ids_of = |group: &[Item]| group.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    // Priority first; then d and e, which conflict with two each, by age.
    assert_eq!(
        ids_of(&inbox.needs_you),
        [a.clone(), d.clone(), e.clone(), c]
    );
    assert_eq!(inbox.conflicts[&b], [d.clone(), e.clone()]);
    assert_eq!(inbox.conflicts[&d], [b, e]);
    assert!(!inbox.conflicts.contains_key(&a));
}

#[test]
fn an_agent_waiting_on_an_answer_puts_its_item_under_needs_you() {
    let mut store = FakeStore::default();
    let asked = add(&mut store, "app", "asked");
    let working = add(&mut store, "app", "working");
    for id in [&asked, &working] {
        store.set_state(id, ItemState::ChangesRequested);
    }
    let comment = store
        .insert_draft(NewComment {
            item: asked.clone(),
            revision: 1,
            path: "src/login.rs".into(),
            side: Side::New,
            start_line: 1,
            line: 1,
            body: "Why here?".into(),
            quote: "fn login()".into(),
        })
        .unwrap();
    store
        .insert_reply(NewReply {
            comment: comment.id,
            item: asked.clone(),
            author: Author::Agent,
            status: Some(ReplyStatus::Question),
            body: "Should it retry?".into(),
            revision: 1,
        })
        .unwrap();

    let inbox = inbox(&store).unwrap();
    let ids = |group: &[Item]| group.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&inbox.needs_you), std::slice::from_ref(&asked));
    assert_eq!(ids(&inbox.with_agents), [working]);
    assert_eq!(inbox.waiting[&asked].questions, 1);
}

#[test]
fn lists_items_closed_today_latest_first() {
    let mut store = FakeStore::default();
    let merged = add(&mut store, "app", "merged");
    let open = add(&mut store, "app", "open");
    let abandoned = add(&mut store, "app", "abandoned");
    store.set_state(&merged, ItemState::Merged);
    store.set_state(&abandoned, ItemState::Abandoned);

    let inbox = inbox(&store).unwrap();
    let ids = |group: &[Item]| group.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&inbox.closed), [abandoned, merged]);
    assert_eq!(ids(&inbox.needs_you), [open]);
}
