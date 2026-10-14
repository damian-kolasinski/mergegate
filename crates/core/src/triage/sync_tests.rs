use std::cell::RefCell;

use super::*;
use crate::fake::{FakeStore, rules};
use crate::item::ItemId;
use crate::triage::{Rule, RuleId, Tier};

/// The file's rules, or the error reading it gives.
struct File(RefCell<Result<Vec<Rule>, RulesFileError>>);

impl File {
    fn holding(rules: &Rules) -> Self {
        Self(RefCell::new(Ok(rules.rules().cloned().collect())))
    }
}

impl RulesFile for File {
    fn read(&self, _project: &ProjectName) -> Result<Vec<Rule>, RulesFileError> {
        self.0.borrow().clone()
    }

    fn location(&self, project: &ProjectName) -> String {
        format!("{project}/rules.toml")
    }
}

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

#[test]
fn applies_new_rules_to_every_revision_of_open_items() {
    // r1 changes README.md and src/login.rs, r2 adds src/r2.rs.
    let mut store = FakeStore::with_item(2);
    let file = File::holding(&rules(&[(Tier::Trusted, "**/*.md")]));
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Applied(vec![id()])
    );
    for revision in [1, 2] {
        let readme = tiers(&store, revision)
            .into_iter()
            .find(|(p, _)| p == "README.md");
        assert_eq!(
            readme,
            Some(("README.md".into(), Tier::Trusted)),
            "r{revision}"
        );
    }
    let stat = store.item(&id()).unwrap().unwrap().revision.stat;
    assert_eq!(stat.review_files + 1, stat.files);
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Unchanged);
}

#[test]
fn rules_that_move_nothing_name_no_items() {
    let mut store = FakeStore::with_item(1);
    let file = File::holding(&rules(&[(Tier::Trusted, "**/*.lock")]));
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Applied(vec![])
    );
    assert_eq!(
        store.applied_rules(&app()).unwrap(),
        rules(&[(Tier::Trusted, "**/*.lock")])
    );
}

#[test]
fn removing_the_rules_reviews_everything_again() {
    let mut store = FakeStore::with_item(1);
    let file = File::holding(&rules(&[(Tier::Trusted, "**")]));
    sync(&mut store, &file, &app()).unwrap();
    *file.0.borrow_mut() = Ok(Vec::new());
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
fn a_bad_file_keeps_the_applied_rules() {
    let mut store = FakeStore::with_item(1);
    let applied = rules(&[(Tier::Trusted, "**/*.md")]);
    sync(&mut store, &File::holding(&applied), &app()).unwrap();
    let unparsable = File(RefCell::new(Err(RulesFileError::Invalid(
        "line 3: expected `=`".into(),
    ))));
    let duplicate = File(RefCell::new(Ok(vec![
        applied.rules().next().unwrap().clone(),
        Rule {
            id: RuleId::new(1),
            ..applied.rules().next().unwrap().clone()
        },
    ])));
    for (file, error) in [
        (unparsable, "line 3: expected `=`"),
        (duplicate, "R1 appears twice"),
    ] {
        let synced = sync(&mut store, &file, &app()).unwrap();
        assert_eq!(synced, Synced::Invalid(error.into()));
        assert_eq!(store.applied_rules(&app()).unwrap(), applied);
    }
}

#[test]
fn closed_items_keep_their_triage() {
    let mut store = FakeStore::with_item(1);
    store.set_state(&id(), crate::item::ItemState::Merged);
    let file = File::holding(&rules(&[(Tier::Trusted, "**")]));
    assert_eq!(
        sync(&mut store, &file, &app()).unwrap(),
        Synced::Applied(vec![])
    );
    assert!(
        tiers(&store, 1)
            .iter()
            .all(|(_, tier)| *tier == Tier::Review)
    );
}
