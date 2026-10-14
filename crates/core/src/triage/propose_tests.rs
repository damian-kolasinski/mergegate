use super::*;
use crate::fake::{FakeRulesFile, FakeStore, rule};
use crate::triage::{Hidden, RulesFileError, decide_edit};

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn new_rule(tier: Tier, path: &str) -> NewRule {
    NewRule {
        tier,
        path: Some(path.into()),
        kind: None,
        max_lines: None,
        diff_matches: None,
        reason: "because".into(),
    }
}

fn readme(store: &FakeStore) -> Tier {
    store
        .revision_files(&id(), 1)
        .unwrap()
        .into_iter()
        .find(|f| f.file.path == "README.md")
        .unwrap()
        .triage
        .tier
}

fn ids(numbers: &[u32]) -> Vec<RuleId> {
    numbers.iter().map(|&n| RuleId::new(n)).collect()
}

#[test]
fn a_review_rule_applies_first_at_once() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    file.edit([&rule(1, Tier::Trusted, "**")]);
    sync(&mut store, &file, &app()).unwrap();
    decide_edit(&mut store, &file, &app(), Decision::Approve).unwrap();
    assert_eq!(readme(&store), Tier::Trusted);

    let proposed = propose(
        &mut store,
        &file,
        &app(),
        new_rule(Tier::Review, "README.md"),
    )
    .unwrap();
    assert_eq!(proposed.proposal.rule.id, RuleId::new(2));
    assert_eq!(proposed.proposal.state, ProposalState::Applied);
    assert_eq!(proposed.moved, [id()]);
    assert_eq!(readme(&store), Tier::Review);
    // Under the operator's comment, which stays.
    assert_eq!(file.text().unwrap(), "# mine\nR2\nR1\n");
    assert_eq!(store.applied_text(&app()).unwrap(), file.text());
    assert_eq!(
        store.proposal(&app(), RuleId::new(2)).unwrap(),
        Some(proposed.proposal)
    );
}

#[test]
fn a_trusted_rule_waits_with_a_preview_and_applies_last() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    let proposed = propose(
        &mut store,
        &file,
        &app(),
        new_rule(Tier::Trusted, "**/*.md"),
    )
    .unwrap();
    let proposal = &proposed.proposal;
    assert_eq!(
        (proposal.rule.id, proposal.state),
        (RuleId::new(1), ProposalState::Pending)
    );
    let preview = proposal.preview.as_ref().unwrap();
    assert_eq!(
        preview.hidden,
        [Hidden {
            item: id(),
            path: "README.md".into(),
            rule: RuleId::new(1),
        }]
    );
    assert_eq!(file.text(), None);
    assert_eq!(readme(&store), Tier::Review);
    assert_eq!(store.pending_proposals(&app()).unwrap().len(), 1);

    file.edit([&rule(2, Tier::Review, "src/**")]);
    let moved =
        decide_proposal(&mut store, &file, &app(), RuleId::new(1), Decision::Approve).unwrap();
    assert_eq!(moved, [id()]);
    assert_eq!(readme(&store), Tier::Trusted);
    assert_eq!(file.ids(), ids(&[2, 1]));
    assert!(store.pending_proposals(&app()).unwrap().is_empty());
    let again = decide_proposal(&mut store, &file, &app(), RuleId::new(1), Decision::Reject);
    assert!(
        matches!(
            again,
            Err(ChangeError::NotPending(_, ProposalState::Applied))
        ),
        "{again:?}"
    );
}

#[test]
fn a_rejected_rule_changes_nothing_and_keeps_its_id() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    propose(&mut store, &file, &app(), new_rule(Tier::Trusted, "**")).unwrap();
    let moved =
        decide_proposal(&mut store, &file, &app(), RuleId::new(1), Decision::Reject).unwrap();
    assert!(moved.is_empty());
    assert_eq!(
        store
            .proposal(&app(), RuleId::new(1))
            .unwrap()
            .unwrap()
            .state,
        ProposalState::Rejected
    );
    assert_eq!(file.text(), None);
    let next = propose(&mut store, &file, &app(), new_rule(Tier::Trusted, "**")).unwrap();
    assert_eq!(next.proposal.rule.id, RuleId::new(2));
}

#[test]
fn ids_skip_those_in_the_file() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    file.edit([&rule(7, Tier::Review, "src/**")]);
    let proposed = propose(&mut store, &file, &app(), new_rule(Tier::Review, "a")).unwrap();
    assert_eq!(proposed.proposal.rule.id, RuleId::new(8));
}

#[test]
fn a_rule_without_a_reason_or_matcher_is_refused() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    let no_matcher = NewRule {
        path: None,
        ..new_rule(Tier::Trusted, "")
    };
    let refused = propose(&mut store, &file, &app(), no_matcher);
    assert!(
        matches!(refused, Err(ChangeError::Rule(RuleError::NoMatcher(_)))),
        "{refused:?}"
    );
    let no_reason = NewRule {
        reason: " ".into(),
        ..new_rule(Tier::Review, "**")
    };
    let refused = propose(&mut store, &file, &app(), no_reason);
    assert!(
        matches!(refused, Err(ChangeError::Rule(RuleError::NoReason(_)))),
        "{refused:?}"
    );
    assert!(store.pending_proposals(&app()).unwrap().is_empty());
}

#[test]
fn a_broken_file_waits_to_be_fixed() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    propose(&mut store, &file, &app(), new_rule(Tier::Trusted, "**")).unwrap();
    file.break_with(RulesFileError::Invalid("line 1: nope".into()));
    let approved = decide_proposal(&mut store, &file, &app(), RuleId::new(1), Decision::Approve);
    assert!(
        matches!(&approved, Err(ChangeError::InvalidFile(e)) if e == "app/rules.toml can't be used: line 1: nope"),
        "{approved:?}"
    );
    assert_eq!(
        store
            .proposal(&app(), RuleId::new(1))
            .unwrap()
            .unwrap()
            .state,
        ProposalState::Pending
    );
}

#[test]
fn approving_beside_a_held_edit_keeps_the_edit_held() {
    let mut store = FakeStore::with_item(1);
    let file = FakeRulesFile::default();
    file.edit([&rule(1, Tier::Review, "src/**")]);
    sync(&mut store, &file, &app()).unwrap();
    file.edit([
        &rule(1, Tier::Review, "src/**"),
        &rule(2, Tier::Trusted, "**"),
    ]);
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Held);

    propose(&mut store, &file, &app(), new_rule(Tier::Trusted, "*.md")).unwrap();
    decide_proposal(&mut store, &file, &app(), RuleId::new(3), Decision::Approve).unwrap();
    assert_eq!(file.ids(), ids(&[1, 2, 3]));
    let applied: Vec<_> = store
        .applied_rules(&app())
        .unwrap()
        .rules()
        .map(|r| r.id)
        .collect();
    assert_eq!(applied, ids(&[1, 3]));
    let held = store.held_edit(&app()).unwrap().unwrap();
    assert_eq!(held.rules.rules().count(), 3);

    // Rejecting the edit keeps the approved rule.
    decide_edit(&mut store, &file, &app(), Decision::Reject).unwrap();
    assert_eq!(file.ids(), ids(&[1, 3]));
    assert_eq!(sync(&mut store, &file, &app()).unwrap(), Synced::Unchanged);
}
