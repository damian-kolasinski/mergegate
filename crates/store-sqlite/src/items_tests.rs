use mergegate_core::item::{ChangedFile, FileStat};
use mergegate_core::project::{Backend, Project, ProjectStore};

use super::*;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    for name in ["app", "web"] {
        let project = Project {
            name: ProjectName::parse(name).unwrap(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: format!("/src/{name}").into(),
        };
        store.insert_project(&project).unwrap();
    }
    (dir, store)
}

fn name(project: &str) -> ProjectName {
    ProjectName::parse(project).unwrap()
}

fn revision(head: char) -> NewRevision {
    NewRevision {
        head: head.to_string().repeat(40),
        base: "0".repeat(40),
        files: vec![
            FileStat {
                file: ChangedFile {
                    path: "src/new.rs".into(),
                    old_path: Some("src/old.rs".into()),
                    old_blob: Some("1".repeat(40)),
                    new_blob: Some(head.to_string().repeat(40)),
                },
                additions: Some(5),
                deletions: Some(2),
            },
            FileStat {
                file: ChangedFile {
                    path: "logo.png".into(),
                    old_path: None,
                    old_blob: None,
                    new_blob: Some("2".repeat(40)),
                },
                additions: None,
                deletions: None,
            },
        ],
    }
}

fn new_item(project: &str, branch: &str) -> NewItem {
    NewItem {
        project: name(project),
        branch: branch.into(),
        title: format!("Work on {branch}"),
        state: ItemState::NeedsReview,
        backend: Backend::Local,
        target: "main".into(),
        revision: revision('a'),
    }
}

#[test]
fn numbers_items_per_project_and_lists_open_ones_oldest_first() {
    let (_dir, mut store) = store();
    let a = store.insert_item(new_item("app", "a")).unwrap();
    let b = store.insert_item(new_item("web", "b")).unwrap();
    let c = store.insert_item(new_item("app", "c")).unwrap();

    assert_eq!(a.id.to_string(), "app#1");
    assert_eq!(b.id.to_string(), "web#1");
    assert_eq!(c.id.to_string(), "app#2");
    assert_eq!(a.title, "Work on a");
    assert_eq!(
        (a.state, a.backend, a.target.as_str()),
        (ItemState::NeedsReview, Backend::Local, "main")
    );
    assert_eq!(a.revision.number, 1);
    assert_eq!(a.revision.head, "a".repeat(40));
    assert_eq!(
        a.revision.stat,
        DiffStat {
            files: 2,
            additions: 5,
            deletions: 2
        }
    );
    assert_eq!(a.created_at, a.revision.submitted_at);
    assert!(a.created_at.ends_with('Z'), "{}", a.created_at);

    assert_eq!(store.open_items(None).unwrap(), [a.clone(), b, c.clone()]);
    assert_eq!(
        store.open_items(Some(&name("app"))).unwrap(),
        [a.clone(), c]
    );
    assert_eq!(store.open_item(&name("app"), "a").unwrap(), Some(a));
    assert_eq!(store.open_item(&name("web"), "a").unwrap(), None);
}

#[test]
fn reads_any_item_by_id() {
    let (_dir, mut store) = store();
    let a = store.insert_item(new_item("app", "a")).unwrap();
    store
        .conn
        .execute("UPDATE items SET state = 'merged'", [])
        .unwrap();
    let merged = store.item(&a.id).unwrap().unwrap();
    assert_eq!(merged.state, ItemState::Merged);
    assert_eq!(merged.revision, a.revision);
    assert_eq!(store.item(&ItemId::new(name("app"), 2)).unwrap(), None);
    assert_eq!(store.item(&ItemId::new(name("web"), 1)).unwrap(), None);
}

#[test]
fn adds_revisions_with_their_files() {
    let (_dir, mut store) = store();
    let first = store.insert_item(new_item("app", "a")).unwrap();
    let item = store
        .add_revision(&first.id, "Renamed", ItemState::NeedsReview, revision('b'))
        .unwrap();
    assert_eq!(item.id, first.id);
    assert_eq!(item.title, "Renamed");
    assert_eq!(item.revision.number, 2);
    assert_eq!(item.revision.head, "b".repeat(40));
    assert_eq!(item.created_at, first.created_at);
    assert_eq!(store.open_items(None).unwrap(), std::slice::from_ref(&item));

    let files: Vec<(i64, String, Option<String>, Option<i64>)> = store
        .conn
        .prepare("SELECT revision, path, old_path, additions FROM revision_files ORDER BY revision, position")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(files.len(), 4);
    assert_eq!(
        files[2],
        (2, "src/new.rs".into(), Some("src/old.rs".into()), Some(5))
    );
    assert_eq!(files[3], (2, "logo.png".into(), None, None));

    let changed = |revision| store.revision_files(&item.id, revision).unwrap();
    let files_of = |head| {
        revision(head)
            .files
            .into_iter()
            .map(|f| f.file)
            .collect::<Vec<_>>()
    };
    assert_eq!(changed(1), files_of('a'));
    assert_eq!(changed(2), files_of('b'));
    assert!(changed(3).is_empty());
}

#[test]
fn allows_one_open_item_per_branch() {
    let (_dir, mut store) = store();
    let first = store.insert_item(new_item("app", "a")).unwrap();
    assert!(store.insert_item(new_item("app", "a")).is_err());

    store
        .conn
        .execute("UPDATE items SET state = 'merged'", [])
        .unwrap();
    assert!(store.open_items(None).unwrap().is_empty());
    let next = store.insert_item(new_item("app", "a")).unwrap();
    assert_eq!(next.id.number(), first.id.number() + 1);
}
