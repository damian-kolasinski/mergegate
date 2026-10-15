use super::*;
use crate::fake::{FakeRulesFile, FakeStore, rule, rules};
use crate::item::ItemId;
use crate::triage::{Decision, Hidden, Preview, Rule, RuleId, Tier, decide_edit};

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn tiers(store: &FakeStore, revision: u32) -> Vec<(String, Tier)> {
    store
        .revision_files(&id(), revision)
        .unwrap()
        .into_iter()
        .map(|f| (f.file.path, f.triage.tier))
        .collect()
}

fn readme(store: &FakeStore, revision: u32) -> Tier {
    tiers(store, revision)
        .into_iter()
        .find(|(p, _)| p == "README.md")
        .unwrap()
        .1
}

/// `rules` written by hand and approved.
fn approved(store: &mut FakeStore, file: &FakeRulesFile, rules: &Rules) {
    file.edit(rules.rules());
    if sync(store, file, &app()).unwrap() == Synced::Held {
        decide_edit(store, file, &app(), Decision::Approve).unwrap();
    }
    assert_eq!(&store.applied_rules(&app()).unwrap(), rules);
}

#[test]
fn an_edit_that_trusts_more_is_held_with_a_preview() {
    // r1 changes README.md and src/login.rs.
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    let edit = rules(&[(Tier::Trusted, "**/*.md")]);
    file.edit(edit.rules());

    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Held);
    assert!(store.applied_rules(&app()).unwrap().is_empty());
    assert_eq!(readme(&store, 1), Tier::Review);
    let held = store.held_edit(&app()).unwrap().unwrap();
    assert_eq!(held.rules, edit);
    assert_eq!(held.text, file.text().unwrap());
    assert_eq!(
        held.preview,
        Preview {
            items: 1,
            hidden: vec![Hidden {
                item: id(),
                path: "README.md".into(),
                rule: RuleId::new(1),
            }],
        }
    );
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Unchanged);
}

#[test]
fn an_edit_that_only_reviews_more_applies_to_every_revision() {
    // r2 adds src/r2.rs.
    let mut store = FakeStore::with_item(2);
    let file = FakeRulesFile::default();
    approved(&mut store, &file, &rules(&[(Tier::Trusted, "**/*.md")]));
    for revision in [1, 2] {
        assert_eq!(readme(&store, revision), Tier::Trusted, "r{revision}");
    }
    let stat = store.item(&id()).unwrap().unwrap().revision.stat;
    assert_eq!(stat.review_files + 1, stat.files);

    let first = rule(2, Tier::Review, "README.md");
    file.edit([&first, &rule(1, Tier::Trusted, "**/*.md")]);
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Applied(vec![id()])
    );
    for revision in [1, 2] {
        assert_eq!(readme(&store, revision), Tier::Review, "r{revision}");
    }
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Unchanged);
}

#[test]
fn removing_trusted_rules_reviews_everything_again() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    approved(&mut store, &file, &rules(&[(Tier::Trusted, "**")]));
    file.edit([]);
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Applied(vec![id()])
    );
    assert!(
        tiers(&store, 1)
            .iter()
            .all(|(_, tier)| *tier == Tier::Review)
    );
}

#[test]
fn removing_a_review_rule_or_reordering_is_held() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    let (review, trusted) = (
        rule(1, Tier::Review, "src/**"),
        rule(2, Tier::Trusted, "**"),
    );
    approved(
        &mut store,
        &file,
        &Rules::new(vec![review.clone()]).unwrap(),
    );
    file.edit([]);
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Held);

    approved(
        &mut store,
        &file,
        &Rules::new(vec![review.clone(), trusted.clone()]).unwrap(),
    );
    file.edit([&trusted, &review]);
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Held);
}

#[test]
fn undoing_a_held_edit_by_hand_drops_it() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    file.edit(rules(&[(Tier::Trusted, "**")]).rules());
    sync(&mut store, &file, &app()).unwrap();
    file.edit([]);
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Applied(vec![])
    );
    assert_eq!(store.held_edit(&app()).unwrap(), None);
}

#[test]
fn a_new_edit_replaces_the_held_one() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    file.edit(rules(&[(Tier::Trusted, "**/*.md")]).rules());
    sync(&mut store, &file, &app()).unwrap();
    let next = rules(&[(Tier::Trusted, "**")]);
    file.edit(next.rules());
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Held);
    let held = store.held_edit(&app()).unwrap().unwrap();
    assert_eq!((held.rules, held.preview.hidden.len()), (next, 2));
}

#[test]
fn a_comment_edit_keeps_the_rules_and_records_the_text() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    approved(&mut store, &file, &rules(&[(Tier::Review, "src/**")]));
    file.write_text("# why we review src\nR1\n");
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Unchanged);
    assert_eq!(
        store.applied_text(&app()).unwrap().as_deref(),
        Some("# why we review src\nR1\n")
    );
}

#[test]
fn a_bad_file_keeps_the_applied_rules() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    let applied = rules(&[(Tier::Trusted, "**/*.md")]);
    approved(&mut store, &file, &applied);

    file.break_with(RulesFileError::Invalid("line 3: expected `=`".into()));
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Invalid("line 3: expected `=`".into())
    );
    let first = applied.rules().next().unwrap();
    file.edit([first, &Rule { ..first.clone() }]);
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Invalid("R1 appears twice".into())
    );
    assert_eq!(store.applied_rules(&app()).unwrap(), applied);
}

#[test]
fn closed_items_keep_their_triage() {
    let mut store = FakeStore::with_item(1);
    store.set_state(&id(), crate::item::ItemState::Merged);
    let file = FakeRulesFile::default();
    file.edit(rules(&[(Tier::Trusted, "**")]).rules());
    sync(&mut store, &file, &app()).unwrap();
    assert_eq!(
        decide_edit(&mut store, &file, &app(), Decision::Approve).unwrap(),
        vec![]
    );
    assert!(
        tiers(&store, 1)
            .iter()
            .all(|(_, tier)| *tier == Tier::Review)
    );
}

#[test]
fn a_hand_added_rule_cant_take_a_retired_id() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    let r1 = rule(1, Tier::Review, "src/**");
    let both = Rules::new(vec![r1.clone(), rule(2, Tier::Review, "*.md")]).unwrap();
    approved(&mut store, &file, &both);
    approved(&mut store, &file, &Rules::new(vec![r1.clone()]).unwrap());

    file.edit([&r1, &rule(2, Tier::Review, "docs/**")]);
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Invalid("R2 is taken by an earlier rule; use R3".into())
    );
    assert_eq!(store.applied_rules(&app()).unwrap().rules().count(), 1);

    file.edit([&r1, &rule(3, Tier::Review, "docs/**")]);
    assert!(matches!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Applied(_)
    ));
}
