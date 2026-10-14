use mergegate_core::item::{
    ChangedFile, FileStat, ItemBackend, ItemState, ItemStore, NewItem, NewRevision,
};
use mergegate_core::project::{Backend, Project, ProjectStore};

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
        path: None,
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
    store.set_applied_rules(&app(), &rules).unwrap();
    assert_eq!(store.applied_rules(&app()).unwrap(), rules);
    let web = ProjectName::parse("web").unwrap();
    assert!(store.applied_rules(&web).unwrap().is_empty());

    let fewer = Rules::new(vec![Rule {
        path: Some("*.md".into()),
        ..rule(4, Tier::Trusted)
    }])
    .unwrap();
    store.set_applied_rules(&app(), &fewer).unwrap();
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
            },
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
