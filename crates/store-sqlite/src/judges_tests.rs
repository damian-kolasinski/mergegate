use mergegate_core::item::{
    ChangedFile, FileMode, FileStat, ItemBackend, ItemState, ItemStore, NewItem, NewRevision,
};
use mergegate_core::project::{Backend, Project, ProjectStore};
use mergegate_core::triage::{OrderBy, Tier, Triage};

use super::*;

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn file(path: &str, tier: Tier) -> FileStat {
    FileStat {
        file: ChangedFile {
            path: path.into(),
            old_path: None,
            old_blob: None,
            new_blob: Some("1".repeat(40)),
            old_mode: None,
            new_mode: Some(FileMode::Regular),
        },
        additions: Some(1),
        deletions: Some(0),
        generated: false,
        triage: Triage { tier, by: None },
    }
}

/// `app#1`, whose one revision has `src/a.rs` to review and `Cargo.lock`
/// trusted.
fn store() -> (tempfile::TempDir, Store, ItemId) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    store
        .insert_project(&Project {
            name: app(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
        })
        .unwrap();
    let item = store
        .insert_item(NewItem {
            project: app(),
            branch: "work".into(),
            title: "Work".into(),
            state: ItemState::NeedsReview,
            backend: ItemBackend::Local,
            target: "main".into(),
            revision: NewRevision {
                head: "a".repeat(40),
                base: "0".repeat(40),
                files: vec![
                    file("Cargo.lock", Tier::Trusted),
                    file("src/a.rs", Tier::Review),
                ],
                guide: None,
                interdiffs: Vec::new(),
                viewed: Vec::new(),
                parent: None,
            },
            priority: None,
            note: None,
        })
        .unwrap();
    (dir, store, item.id)
}

fn jev(model: &str, threshold: f64) -> JudgeSetting {
    JudgeSetting {
        provider: Provider::Jev {
            model: model.into(),
        },
        threshold,
        order: OrderBy::Guide,
    }
}

fn verdict(path: &str, importance: u8, promote: Option<f64>) -> Verdict {
    Verdict {
        path: path.into(),
        importance,
        reason: Some("why".into()),
        promote,
    }
}

#[test]
fn keeps_each_projects_judge() {
    let (_dir, mut store, _) = store();
    assert_eq!(store.judge_setting(&app()).unwrap(), None);
    store
        .set_judge_setting(&app(), Some(&jev("jev-1", 0.8)))
        .unwrap();
    assert_eq!(
        store.judge_setting(&app()).unwrap(),
        Some(jev("jev-1", 0.8))
    );
    let command = JudgeSetting {
        provider: Provider::Command {
            command: "claude -p".into(),
        },
        threshold: 0.5,
        order: OrderBy::Judge,
    };
    store.set_judge_setting(&app(), Some(&command)).unwrap();
    assert_eq!(store.judge_setting(&app()).unwrap(), Some(command));
    store.set_judge_setting(&app(), None).unwrap();
    assert_eq!(store.judge_setting(&app()).unwrap(), None);
}

#[test]
fn reads_verdicts_only_under_the_key_that_gave_them() {
    let (_dir, mut store, id) = store();
    assert_eq!(store.verdicts(&id, 1, "jev jev-1").unwrap(), None);
    store.insert_verdicts(&id, 1, "jev jev-1", &[]).unwrap();
    assert_eq!(store.verdicts(&id, 1, "jev jev-1").unwrap(), Some(vec![]));

    let given = [
        verdict("src/a.rs", 70, None),
        verdict("Cargo.lock", 5, Some(0.25)),
    ];
    store.insert_verdicts(&id, 1, "jev jev-2", &given).unwrap();
    assert_eq!(
        store.verdicts(&id, 1, "jev jev-1").unwrap(),
        None,
        "replaced"
    );
    assert_eq!(
        store.verdicts(&id, 1, "jev jev-2").unwrap(),
        Some(vec![given[1].clone(), given[0].clone()])
    );
    assert_eq!(store.verdicts(&id, 2, "jev jev-2").unwrap(), None);
}

#[test]
fn counts_a_promoted_file_as_one_to_review() {
    let (_dir, mut store, id) = store();
    let review_files = |store: &Store| store.item(&id).unwrap().unwrap().revision.stat.review_files;
    assert_eq!(review_files(&store), 1);

    store
        .set_judge_setting(&app(), Some(&jev("jev-1", 0.8)))
        .unwrap();
    let promoting = [verdict("Cargo.lock", 5, Some(0.8))];
    store
        .insert_verdicts(&id, 1, "jev jev-2", &promoting)
        .unwrap();
    assert_eq!(review_files(&store), 1, "another judge's");

    store
        .insert_verdicts(&id, 1, "jev jev-1", &promoting)
        .unwrap();
    assert_eq!(review_files(&store), 2);
    assert_eq!(store.revisions(&id).unwrap()[0].stat.review_files, 2);
    assert_eq!(
        store.open_items(None).unwrap()[0]
            .revision
            .stat
            .review_files,
        2
    );

    store
        .set_judge_setting(&app(), Some(&jev("jev-1", 0.9)))
        .unwrap();
    assert_eq!(review_files(&store), 1, "under the threshold");
}
