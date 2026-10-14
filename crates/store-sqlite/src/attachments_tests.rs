use mergegate_core::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};

use super::*;

/// `app#1` with two revisions and `web#1` with one.
fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    let revision = || NewRevision {
        head: "a".repeat(40),
        base: "0".repeat(40),
        files: Vec::new(),
    };
    for name in ["app", "web"] {
        let name = ProjectName::parse(name).unwrap();
        store
            .insert_project(&Project {
                name: name.clone(),
                backend: Backend::Local,
                target: "main".into(),
                upstream: format!("/src/{name}").into(),
                github: None,
            })
            .unwrap();
        store
            .insert_item(NewItem {
                project: name,
                branch: "work".into(),
                title: "Work".into(),
                state: ItemState::NeedsReview,
                backend: ItemBackend::Local,
                target: "main".into(),
                revision: revision(),
            })
            .unwrap();
    }
    store
        .add_revision(&id("app#1"), "Work", ItemState::NeedsReview, revision())
        .unwrap();
    (dir, store)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn add(store: &mut Store, item: &str, revision: u32, name: &str, bytes: &[u8]) -> Attachment {
    store
        .add_attachment(
            &id(item),
            NewAttachment {
                revision,
                name,
                media: Media::Png,
                bytes,
            },
        )
        .unwrap()
}

fn abandon(store: &mut Store, item: &str) {
    let notice = Notice {
        revision: 1,
        source: Source::Agent,
        kind: NoticeKind::Abandoned { reason: None },
    };
    store
        .notify(&id(item), ItemState::Abandoned, notice)
        .unwrap();
}

fn files(dir: &tempfile::TempDir) -> Vec<String> {
    let mut names: Vec<String> = match std::fs::read_dir(dir.path().join("attachments")) {
        Ok(entries) => entries
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names
}

#[test]
fn lists_attachments_by_revision_then_the_order_added() {
    let (dir, mut store) = store();
    let b = add(&mut store, "app#1", 2, "b.png", b"second");
    let a = add(&mut store, "app#1", 1, "a.png", b"first");
    let c = add(&mut store, "app#1", 2, "c.png", b"third");
    add(&mut store, "web#1", 1, "w.png", b"web");
    assert_eq!(store.attachments(&id("app#1")).unwrap(), [a, b.clone(), c]);
    assert_eq!(
        b.sha256,
        "16367aacb67a4a017c8da8ab95682ccb390863780f7114dda0a0e0c55644c7c4"
    );
    assert_eq!(b.size, 6);
    assert_eq!(
        std::fs::read(store.attachment_file(&b.sha256)).unwrap(),
        b"second"
    );
    assert_eq!(files(&dir).len(), 4);
}

#[test]
fn keeps_one_file_for_the_same_content() {
    let (dir, mut store) = store();
    let a = add(&mut store, "app#1", 1, "a.png", b"same");
    let b = add(&mut store, "web#1", 1, "b.png", b"same");
    assert_eq!(a.sha256, b.sha256);
    assert_eq!(files(&dir), std::slice::from_ref(&a.sha256));
    assert_eq!(store.attachment_media(&a.sha256).unwrap(), Some(Media::Png));
    assert_eq!(store.attachment_media(&"0".repeat(64)).unwrap(), None);
}

#[test]
fn refuses_a_revision_the_item_does_not_have() {
    let (_dir, mut store) = store();
    let missing = store.add_attachment(
        &id("web#1"),
        NewAttachment {
            revision: 2,
            name: "a.png",
            media: Media::Png,
            bytes: b"x",
        },
    );
    assert!(missing.is_err());
}

#[test]
fn prunes_closed_items_once_their_time_is_up() {
    let (dir, mut store) = store();
    let shared = add(&mut store, "app#1", 1, "a.png", b"shared");
    let own = add(&mut store, "app#1", 2, "b.png", b"app only");
    let open = add(&mut store, "web#1", 1, "w.png", b"shared");
    std::fs::write(dir.path().join("attachments/stray.tmp"), b"crash").unwrap();

    abandon(&mut store, "app#1");
    assert_eq!(
        store.prune_attachments(Duration::from_secs(3600)).unwrap(),
        []
    );
    assert_eq!(store.attachments(&id("app#1")).unwrap().len(), 2);
    assert_eq!(files(&dir).len(), 2, "the stray file is swept");

    assert_eq!(
        store.prune_attachments(Duration::ZERO).unwrap(),
        [id("app#1")]
    );
    assert_eq!(store.attachments(&id("app#1")).unwrap(), []);
    assert_eq!(store.attachments(&id("web#1")).unwrap(), [open]);
    assert_eq!(files(&dir), [shared.sha256], "{} is gone", own.sha256);
}

#[test]
fn a_close_starts_the_clock_once() {
    let (_dir, mut store) = store();
    let closed_at = |store: &Store| -> Option<String> {
        store
            .conn
            .query_row("SELECT closed_at FROM items WHERE number = 1 AND project_id = (SELECT id FROM projects WHERE name = 'app')", [], |row| row.get(0))
            .unwrap()
    };
    assert_eq!(closed_at(&store), None);
    abandon(&mut store, "app#1");
    let first = closed_at(&store).unwrap();
    abandon(&mut store, "app#1");
    assert_eq!(closed_at(&store), Some(first));
}
