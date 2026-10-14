use super::*;
use crate::fake::{FakeRulesFile, FakeStore, rule, rules};
use crate::triage::{Synced, Tier, sync};

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn tiers(store: &FakeStore) -> Vec<Tier> {
    store
        .revision_files(&id(), 1)
        .unwrap()
        .into_iter()
        .map(|f| f.triage.tier)
        .collect()
}

#[test]
fn approving_applies_the_held_rules() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    let edit = rules(&[(Tier::Trusted, "**")]);
    file.edit(edit.rules());
    sync(&mut store, &file, &app()).unwrap();

    let moved = decide_edit(&mut store, &file, &app(), Decision::Approve).unwrap();
    assert_eq!(moved, [id()]);
    assert_eq!(tiers(&store), [Tier::Trusted, Tier::Trusted]);
    assert_eq!(store.applied_rules(&app()).unwrap(), edit);
    assert_eq!(store.applied_text(&app()).unwrap(), file.text());
    assert_eq!(store.held_edit(&app()).unwrap(), None);
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Unchanged);
}

#[test]
fn rejecting_writes_the_applied_text_back() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    file.edit([&rule(1, Tier::Review, "src/**")]);
    sync(&mut store, &file, &app()).unwrap();
    let applied = file.text();
    file.edit([&rule(2, Tier::Trusted, "**")]);
    sync(&mut store, &file, &app()).unwrap();

    let moved = decide_edit(&mut store, &file, &app(), Decision::Reject).unwrap();
    assert!(moved.is_empty());
    assert_eq!(file.text(), applied);
    assert_eq!(store.held_edit(&app()).unwrap(), None);
    assert_eq!(tiers(&store), [Tier::Review, Tier::Review]);
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Unchanged);
}

#[test]
fn nothing_held_is_an_error() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    let decided = decide_edit(&mut store, &file, &app(), Decision::Approve);
    assert!(matches!(decided, Err(ChangeError::NoEdit)), "{decided:?}");
}
