use super::*;
use crate::fake::{FakeStore, rules};
use crate::item::ItemState;

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn hidden(preview: &Preview) -> Vec<(&str, u32)> {
    preview
        .hidden
        .iter()
        .map(|h| (h.path.as_str(), h.rule.number()))
        .collect()
}

#[test]
fn counts_the_newest_revisions_files_the_change_would_hide() {
    // r2 changes README.md, src/login.rs and src/r2.rs.
    let store = FakeStore::with_item(2);
    let applied = rules(&[(Tier::Trusted, "README.md")]);
    let next = rules(&[(Tier::Trusted, "README.md"), (Tier::Trusted, "src/**")]);
    let preview = preview(&store, &app(), &applied, &next).unwrap();
    assert_eq!(hidden(&preview), [("src/login.rs", 2), ("src/r2.rs", 2)]);
    assert_eq!((preview.items, preview.items_hit()), (1, 1));
    assert!(preview.hidden.iter().all(|h| h.item == id()));
}

#[test]
fn looks_at_merged_items_but_not_abandoned_ones() {
    let mut store = FakeStore::with_item(1);
    let next = rules(&[(Tier::Trusted, "**")]);
    store.set_state(&id(), ItemState::Merged);
    let merged = preview(&store, &app(), &Rules::default(), &next).unwrap();
    assert_eq!((merged.items, merged.hidden.len()), (1, 2));

    store.set_state(&id(), ItemState::Abandoned);
    let abandoned = preview(&store, &app(), &Rules::default(), &next).unwrap();
    assert_eq!(abandoned, Preview::default());
}
