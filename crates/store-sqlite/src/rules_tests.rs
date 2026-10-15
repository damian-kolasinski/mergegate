use mergegate_core::item::{
    ChangedFile, FileStat, ItemBackend, ItemState, ItemStore, NewItem, NewRevision,
};
use mergegate_core::project::{Backend, Project, ProjectStore};
use mergegate_core::triage::Preview;

use super::*;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    for name in ["app", "web"] {
        store
            .insert_project(&Project {
                name: ProjectName::parse(name).unwrap(),
                backend: Backend::Local,
                target: "main".into(),
                upstream: format!("/src/{name}").into(),
                github: None,
                bots: Vec::new(),
            })
            .unwrap();
    }
    (dir, store)
}

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn rule(id: u32, tier: Tier) -> Rule {
    Rule {
        id: RuleId::new(id),
        tier,
        path: Some(format!("r{id}/**")),
        kind: None,
        max_lines: None,
        diff_matches: None,
        reason: format!("rule {id}"),
    }
}

fn file(path: &str, lines: u64, triage: Triage) -> FileStat {
    FileStat {
        file: ChangedFile {
            path: path.into(),
            old_path: None,
            old_blob: None,
            new_blob: Some("1".repeat(40)),
        },
        additions: Some(lines),
        deletions: Some(1),
        generated: path.ends_with(".lock"),
        triage,
    }
}

fn trusted_by(id: u32) -> Triage {
    Triage {
        tier: Tier::Trusted,
        by: Some(TriagedBy::Rule {
            id: RuleId::new(id),
            why: "`*.lock` — rule".into(),
        }),
    }
}

#[test]
fn keeps_each_projects_rules_in_order() {
    let (_dir, mut store) = store();
    assert!(store.applied_rules(&app()).unwrap().is_empty());
    let rules = Rules::new(vec![
        Rule {
            path: Some("**/*.lock".into()),
            kind: Some(Kind::Generated),
            max_lines: Some(20),
            diff_matches: Some("^version".into()),
            ..rule(3, Tier::Trusted)
        },
        Rule {
            path: Some("src/**".into()),
            ..rule(1, Tier::Review)
        },
    ])
    .unwrap();
    store
        .set_applied_rules(&app(), &rules, Some("text"))
        .unwrap();
    assert_eq!(store.applied_rules(&app()).unwrap(), rules);
    let web = ProjectName::parse("web").unwrap();
    assert!(store.applied_rules(&web).unwrap().is_empty());

    let fewer = Rules::new(vec![Rule {
        path: Some("*.md".into()),
        ..rule(4, Tier::Trusted)
    }])
    .unwrap();
    store.set_applied_rules(&app(), &fewer, None).unwrap();
    assert_eq!(store.applied_rules(&app()).unwrap(), fewer);
}

#[test]
fn stores_each_files_triage_and_counts_only_reviewed_files() {
    let (_dir, mut store) = store();
    let guarded = Triage {
        tier: Tier::Review,
        by: Some(TriagedBy::Guard(Guard::Gitattributes)),
    };
    let files = vec![
        file(".gitattributes", 1, guarded.clone()),
        file("Cargo.lock", 40, trusted_by(2)),
        file("src/a.rs", 5, Triage::default()),
    ];
    let item = store
        .insert_item(NewItem {
            project: app(),
            branch: "a".into(),
            title: "A".into(),
            state: ItemState::NeedsReview,
            backend: ItemBackend::Local,
            target: "main".into(),
            revision: NewRevision {
                head: "a".repeat(40),
                base: "0".repeat(40),
                files: files.clone(),
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
            },
            priority: None,
            note: None,
        })
        .unwrap();
    assert_eq!(store.revision_files(&item.id, 1).unwrap(), files);
    let stat = item.revision.stat;
    assert_eq!((stat.additions, stat.deletions), (46, 3));
    assert_eq!((stat.files, stat.review_files), (3, 2));

    store
        .set_triage(
            &item.id,
            1,
            &[
                ("Cargo.lock".into(), Triage::default()),
                ("src/a.rs".into(), trusted_by(7)),
            ],
        )
        .unwrap();
    let triage: Vec<_> = store
        .revision_files(&item.id, 1)
        .unwrap()
        .into_iter()
        .map(|f| f.triage)
        .collect();
    assert_eq!(triage, [guarded, Triage::default(), trusted_by(7)]);
    let stat = store.item(&item.id).unwrap().unwrap().revision.stat;
    assert_eq!(stat.review_files, 2);

    store
        .set_triage(&item.id, 1, &[("src/a.rs".into(), Triage::default())])
        .unwrap();
    let stat = store.item(&item.id).unwrap().unwrap().revision.stat;
    assert_eq!(stat.review_files, 3);
}

fn insert(store: &mut Store, branch: &str) -> ItemId {
    store
        .insert_item(NewItem {
            project: app(),
            branch: branch.into(),
            title: branch.to_uppercase(),
            state: ItemState::NeedsReview,
            backend: ItemBackend::Local,
            target: "main".into(),
            revision: NewRevision {
                head: "a".repeat(40),
                base: "0".repeat(40),
                files: vec![file("Cargo.lock", 4, Triage::default())],
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
            },
            priority: None,
            note: None,
        })
        .unwrap()
        .id
}

fn hidden(item: &ItemId, rule: u32) -> Hidden {
    Hidden {
        item: item.clone(),
        path: "Cargo.lock".into(),
        rule: RuleId::new(rule),
    }
}

#[test]
fn keeps_proposals_with_their_previews() {
    let (_dir, mut store) = store();
    let (a, b) = (insert(&mut store, "a"), insert(&mut store, "b"));
    assert_eq!(store.proposal(&app(), RuleId::new(1)).unwrap(), None);
    let review = Proposal {
        rule: rule(1, Tier::Review),
        state: ProposalState::Applied,
        preview: None,
    };
    let trusted = Proposal {
        rule: Rule {
            path: Some("*.lock".into()),
            ..rule(2, Tier::Trusted)
        },
        state: ProposalState::Pending,
        preview: Some(Preview {
            items: 2,
            hidden: vec![hidden(&b, 2), hidden(&a, 2)],
        }),
    };
    store.insert_proposal(&app(), &review).unwrap();
    store.insert_proposal(&app(), &trusted).unwrap();

    assert_eq!(
        store.proposal(&app(), RuleId::new(1)).unwrap(),
        Some(review)
    );
    assert_eq!(
        store.proposal(&app(), RuleId::new(2)).unwrap(),
        Some(trusted.clone())
    );
    assert_eq!(
        store.pending_proposals(&app()).unwrap(),
        std::slice::from_ref(&trusted)
    );
    let web = ProjectName::parse("web").unwrap();
    assert!(store.pending_proposals(&web).unwrap().is_empty());

    store
        .set_proposal_state(&app(), RuleId::new(2), ProposalState::Rejected)
        .unwrap();
    assert!(store.pending_proposals(&app()).unwrap().is_empty());
    let rejected = store.proposal(&app(), RuleId::new(2)).unwrap().unwrap();
    assert_eq!(rejected.state, ProposalState::Rejected);
    assert_eq!(rejected.preview, trusted.preview);
    assert!(
        store
            .set_proposal_state(&app(), RuleId::new(9), ProposalState::Applied)
            .is_err()
    );
}

#[test]
fn keeps_one_held_edit_per_project() {
    let (_dir, mut store) = store();
    let a = insert(&mut store, "a");
    assert_eq!(store.held_edit(&app()).unwrap(), None);
    let edit = |id: u32| HeldEdit {
        rules: Rules::new(vec![rule(1, Tier::Review), rule(id, Tier::Trusted)]).unwrap(),
        text: format!("rules with R{id}"),
        preview: Preview {
            items: 1,
            hidden: vec![hidden(&a, id)],
        },
    };
    store.set_held_edit(&app(), Some(&edit(2))).unwrap();
    assert_eq!(store.held_edit(&app()).unwrap(), Some(edit(2)));

    // A proposal's preview survives replacing the edit.
    let proposal = Proposal {
        rule: rule(5, Tier::Trusted),
        state: ProposalState::Pending,
        preview: Some(Preview {
            items: 1,
            hidden: vec![hidden(&a, 5)],
        }),
    };
    store.insert_proposal(&app(), &proposal).unwrap();
    store.set_held_edit(&app(), Some(&edit(3))).unwrap();
    assert_eq!(store.held_edit(&app()).unwrap(), Some(edit(3)));
    store.set_held_edit(&app(), None).unwrap();
    assert_eq!(store.held_edit(&app()).unwrap(), None);
    assert_eq!(
        store.proposal(&app(), RuleId::new(5)).unwrap(),
        Some(proposal)
    );
}

#[test]
fn numbers_a_new_rule_past_every_one_used() {
    let (_dir, mut store) = store();
    assert_eq!(store.next_rule_id(&app()).unwrap(), RuleId::new(1));
    let rules = Rules::new(vec![rule(3, Tier::Review)]).unwrap();
    store.set_applied_rules(&app(), &rules, None).unwrap();
    assert_eq!(store.next_rule_id(&app()).unwrap(), RuleId::new(4));

    let held = HeldEdit {
        rules: Rules::new(vec![rule(6, Tier::Trusted)]).unwrap(),
        text: String::new(),
        preview: Preview {
            items: 0,
            hidden: Vec::new(),
        },
    };
    store.set_held_edit(&app(), Some(&held)).unwrap();
    store.set_held_edit(&app(), None).unwrap();
    assert_eq!(store.next_rule_id(&app()).unwrap(), RuleId::new(7));

    let proposal = Proposal {
        rule: rule(7, Tier::Trusted),
        state: ProposalState::Rejected,
        preview: None,
    };
    store.insert_proposal(&app(), &proposal).unwrap();
    // Retiring rules doesn't free their numbers.
    store
        .set_applied_rules(&app(), &Rules::new(Vec::new()).unwrap(), Some(""))
        .unwrap();
    assert_eq!(store.next_rule_id(&app()).unwrap(), RuleId::new(8));
    let web = ProjectName::parse("web").unwrap();
    assert_eq!(store.next_rule_id(&web).unwrap(), RuleId::new(1));
}

#[test]
fn keeps_the_text_its_rules_were_applied_from() {
    let (_dir, mut store) = store();
    assert_eq!(store.applied_text(&app()).unwrap(), None);
    let rules = Rules::new(vec![rule(1, Tier::Review)]).unwrap();
    store
        .set_applied_rules(&app(), &rules, Some("one"))
        .unwrap();
    assert_eq!(store.applied_text(&app()).unwrap().as_deref(), Some("one"));
    store.set_applied_rules(&app(), &rules, None).unwrap();
    assert_eq!(store.applied_text(&app()).unwrap(), None);
}
