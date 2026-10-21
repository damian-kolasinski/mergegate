use super::*;
use crate::Store;

fn open_with(path: &Path, migrations: &[&str]) -> Result<Store, StoreError> {
    Store::open_with(path, migrations)
}

fn tables(path: &Path) -> Vec<String> {
    let conn = Connection::open(path).unwrap();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
        .unwrap();
    stmt.query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn version_of(path: &Path) -> u32 {
    user_version(&Connection::open(path).unwrap()).unwrap()
}

#[test]
fn new_database_is_migrated_without_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    let store = Store::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    drop(store);
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(!names.iter().any(|n| n.contains(".bak-")), "{names:?}");
}

#[test]
fn upgrades_from_every_earlier_schema_version() {
    for from in 0..SCHEMA_VERSION {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("hub.db");
        drop(open_with(&db, &MIGRATIONS[..from as usize]).unwrap());
        let store = Store::open(&db).unwrap();
        assert_eq!(
            store.schema_version().unwrap(),
            SCHEMA_VERSION,
            "from {from}"
        );
        if from > 0 {
            assert_eq!(
                version_of(&backup_path(&db, from)),
                from,
                "backup from {from}"
            );
        }
    }
}

#[test]
fn backs_up_before_migrating_an_existing_database() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    drop(open_with(&db, &["CREATE TABLE a (x)"]).unwrap());
    drop(open_with(&db, &["CREATE TABLE a (x)", "CREATE TABLE b (y)"]).unwrap());

    assert_eq!(version_of(&db), 2);
    assert_eq!(tables(&db), ["a", "b"]);
    let backup = backup_path(&db, 1);
    assert_eq!(backup.file_name().unwrap(), "hub.db.bak-1");
    assert_eq!(version_of(&backup), 1);
    assert_eq!(tables(&backup), ["a"]);
}

#[test]
fn replaces_an_earlier_backup_of_the_same_version() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    std::fs::write(backup_path(&db, 1), "stale").unwrap();
    drop(open_with(&db, &["CREATE TABLE a (x)"]).unwrap());
    drop(open_with(&db, &["CREATE TABLE a (x)", "CREATE TABLE b (y)"]).unwrap());
    assert_eq!(tables(&backup_path(&db, 1)), ["a"]);
}

#[test]
fn failed_migration_leaves_the_schema_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    drop(open_with(&db, &["CREATE TABLE a (x)"]).unwrap());
    let err = open_with(&db, &["CREATE TABLE a (x)", "CREATE TABLE b (y); NOT SQL"])
        .err()
        .unwrap();
    assert!(
        matches!(err, StoreError::Migration { version: 2, .. }),
        "{err}"
    );
    assert_eq!(version_of(&db), 1);
    assert_eq!(tables(&db), ["a"]);
}

#[test]
fn refuses_a_database_newer_than_this_build() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    drop(open_with(&db, &["CREATE TABLE a (x)", "CREATE TABLE b (y)"]).unwrap());
    let err = open_with(&db, &["CREATE TABLE a (x)"]).err().unwrap();
    assert!(matches!(
        err,
        StoreError::SchemaTooNew {
            found: 2,
            supported: 1
        }
    ));
}

#[test]
fn a_migration_that_breaks_a_foreign_key_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    let parent = "CREATE TABLE a (id INTEGER PRIMARY KEY);
                  CREATE TABLE b (a_id INTEGER REFERENCES a (id));
                  INSERT INTO a VALUES (1); INSERT INTO b VALUES (1);";
    drop(open_with(&db, &[parent]).unwrap());
    let err = open_with(&db, &[parent, "DELETE FROM a"]).err().unwrap();
    assert!(
        matches!(err, StoreError::Migration { version: 2, .. }),
        "{err}"
    );
    assert_eq!(version_of(&db), 1);
    let conn = Connection::open(&db).unwrap();
    let left: i64 = conn
        .query_row("SELECT count(*) FROM a", [], |row| row.get(0))
        .unwrap();
    assert_eq!(left, 1);
}

#[test]
fn schema_1_data_survives_the_external_review_rebuilds() {
    use mergegate_core::feedback::{EventKind, FeedbackStore, Notice, NoticeKind, Source};
    use mergegate_core::item::{ItemId, ItemState};
    use mergegate_core::review::{
        Author, CommentId, NewComment, NewReply, ReplyStatus, ReviewStore, Side,
    };

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    drop(open_with(&db, &MIGRATIONS[..1]).unwrap());
    let (head, base, at) = ("a".repeat(40), "0".repeat(40), "2026-01-01T00:00:00.000Z");
    // Each table's newest row is deleted, so its sequence is ahead of its
    // rows and must carry over.
    Connection::open(&db)
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO projects (id, name, backend, target, upstream)
                 VALUES (1, 'app', 'local', 'main', '/src/app');
             INSERT INTO items (id, project_id, number, branch, title, state, backend, target,
                                created_at)
                 VALUES (1, 1, 1, 'work', 'Work', 'changes_requested', 'local', 'main', '{at}');
             INSERT INTO revisions (item_id, number, head, base, files, additions, deletions,
                                    submitted_at)
                 VALUES (1, 1, '{head}', '{base}', 0, 0, 0, '{at}');
             INSERT INTO reviews (id, item_id, revision, verdict, summary, submitted_at)
                 VALUES (1, 1, 1, 'changes_requested', 'Fix it', '{at}');
             INSERT INTO comments (item_id, revision, review_id, path, side, start_line, line,
                                   body, quote, created_at)
                 VALUES (1, 1, 1, 'a.rs', 'new', 1, 2, 'Leaks', 'fn a() {{}}', '{at}'),
                        (1, 1, NULL, 'a.rs', 'new', 1, 1, 'Gone', 'fn a() {{', '{at}');
             DELETE FROM comments WHERE id = 2;
             INSERT INTO replies (comment_id, author, status, revision, body, created_at)
                 VALUES (1, 'agent', 'rejected', 1, 'It is closed', '{at}'),
                        (1, 'operator', NULL, 1, 'Gone', '{at}');
             DELETE FROM replies WHERE id = 2;
             INSERT INTO events (item_id, type, revision, source, review_id)
                 VALUES (1, 'review.submitted', 1, 'operator', 1);
             INSERT INTO events (item_id, type, revision, source, body)
                 VALUES (1, 'merge.held', 1, 'operator', 'Gone');
             DELETE FROM events WHERE cursor = 2;"
        ))
        .unwrap();

    let mut store = Store::open(&db).unwrap();
    let app = ItemId::parse("app#1").unwrap();
    let comments = store.comments(&app).unwrap();
    assert_eq!(comments.len(), 1);
    let anchor = comments[0].anchor.as_ref().unwrap();
    assert_eq!((anchor.side, anchor.line), (Side::New, 2));
    assert_eq!(comments[0].review, Some(1));
    assert_eq!(comments[0].external, None);
    let replies = store.replies(&app).unwrap();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].status, Some(ReplyStatus::Rejected));
    assert!(!replies[0].overrides);
    let events = store.events(&app, 0).unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0].kind, EventKind::ReviewSubmitted(r) if r.comments == comments));

    // IDs a deleted row had aren't given out again.
    let draft = NewComment {
        item: app.clone(),
        revision: 1,
        path: "a.rs".into(),
        side: Side::New,
        start_line: 1,
        line: 1,
        body: "Again".into(),
        quote: "fn a() {".into(),
    };
    assert_eq!(store.insert_draft(draft).unwrap().id, CommentId(3));
    let reply = NewReply {
        comment: CommentId(1),
        item: app.clone(),
        author: Author::Operator,
        status: None,
        body: "Why?".into(),
        revision: 1,
    };
    assert_eq!(store.insert_reply(reply).unwrap().id, 3);
    let held = Notice {
        revision: 1,
        source: Source::Operator,
        kind: NoticeKind::MergeHeld {
            reason: "Wait".into(),
        },
    };
    let event = store
        .notify(&app, ItemState::ChangesRequested, held)
        .unwrap();
    assert_eq!(event.cursor, 3);
}

#[test]
fn a_file_recorded_before_modes_were_kept_has_none() {
    use mergegate_core::item::{ItemId, ItemStore};

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    drop(open_with(&db, &MIGRATIONS[..12]).unwrap());
    let (head, base, at) = ("a".repeat(40), "0".repeat(40), "2026-01-01T00:00:00.000Z");
    Connection::open(&db)
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO projects (id, name, backend, target, upstream)
                 VALUES (1, 'app', 'local', 'main', '/src/app');
             INSERT INTO items (id, project_id, number, branch, title, state, backend, target,
                                created_at)
                 VALUES (1, 1, 1, 'work', 'Work', 'needs_review', 'local', 'main', '{at}');
             INSERT INTO revisions (item_id, number, head, base, files, additions, deletions,
                                    submitted_at)
                 VALUES (1, 1, '{head}', '{base}', 1, 0, 0, '{at}');
             INSERT INTO revision_files (item_id, revision, position, path, old_blob, new_blob)
                 VALUES (1, 1, 0, 'run.sh', '{base}', '{base}');"
        ))
        .unwrap();

    let store = Store::open(&db).unwrap();
    let files = store
        .revision_files(&ItemId::parse("app#1").unwrap(), 1)
        .unwrap();
    assert_eq!(
        (files[0].file.old_mode, files[0].file.new_mode),
        (None, None)
    );
    let refused = store
        .conn
        .execute("UPDATE revision_files SET new_mode = '040000'", []);
    assert!(refused.is_err(), "a tree isn't a file's mode");
}

#[test]
fn a_file_recorded_changed_since_a_revision_without_it_reads_as_added() {
    use mergegate_core::interdiff::{Change, InterdiffStore};
    use mergegate_core::item::ItemId;

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    drop(open_with(&db, &MIGRATIONS[..14]).unwrap());
    let (base, at) = ("0".repeat(40), "2026-01-01T00:00:00.000Z");
    let (r1, r2) = ("1".repeat(40), "2".repeat(40));
    Connection::open(&db)
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO projects (id, name, backend, target, upstream)
                 VALUES (1, 'app', 'local', 'main', '/src/app');
             INSERT INTO items (id, project_id, number, branch, title, state, backend, target,
                                created_at)
                 VALUES (1, 1, 1, 'work', 'Work', 'needs_review', 'local', 'main', '{at}');
             INSERT INTO revisions (item_id, number, head, base, files, additions, deletions,
                                    submitted_at)
                 VALUES (1, 1, '{r1}', '{base}', 0, 0, 0, '{at}'),
                        (1, 2, '{r2}', '{base}', 0, 0, 0, '{at}');
             INSERT INTO interdiffs (item_id, from_revision, to_revision) VALUES (1, 1, 2);
             INSERT INTO interdiff_files (item_id, from_revision, to_revision, position, path,
                                          old_blob, new_blob, change)
                 VALUES (1, 1, 2, 0, 'a.py', 'a1', 'a2', 'changed'),
                        (1, 1, 2, 1, 'b.py', NULL, 'b2', 'changed'),
                        (1, 1, 2, 2, 'c.py', 'c1', NULL, 'changed'),
                        (1, 1, 2, 3, 'd.py', 'd1', NULL, 'dropped'),
                        (1, 1, 2, 4, 'e.py', NULL, 'e2', 'unchanged');"
        ))
        .unwrap();

    let store = Store::open(&db).unwrap();
    let changes: Vec<_> = store
        .interdiff(&ItemId::parse("app#1").unwrap(), 1, 2)
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|f| (f.stat.file.path, f.change))
        .collect();
    assert_eq!(
        changes,
        [
            ("a.py".to_owned(), Change::Changed),
            ("b.py".to_owned(), Change::Added),
            ("c.py".to_owned(), Change::Deleted),
            ("d.py".to_owned(), Change::Dropped),
            ("e.py".to_owned(), Change::Unchanged),
        ]
    );
}

#[test]
fn an_approval_carried_before_it_was_marked_ends_no_turn() {
    use mergegate_core::item::ItemId;
    use mergegate_core::review_time::ReviewTimeStore;

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    drop(open_with(&db, &MIGRATIONS[..25]).unwrap());
    let (base, at) = ("0".repeat(40), "2026-01-01T00:00:00.000Z");
    let revisions = (1..=13)
        .map(|n| format!("(1, {n}, '{n:040}', '{base}', 0, 0, 0, '{at}')"))
        .collect::<Vec<_>>()
        .join(", ");
    Connection::open(&db)
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO projects (id, name, backend, target, upstream)
                 VALUES (1, 'app', 'local', 'main', '/src/app');
             INSERT INTO items (id, project_id, number, branch, title, state, backend, target,
                                created_at)
                 VALUES (1, 1, 1, 'work', 'Work', 'merging', 'local', 'main', '{at}');
             INSERT INTO revisions (item_id, number, head, base, files, additions, deletions,
                                    submitted_at)
                 VALUES {revisions};
             INSERT INTO reviews (item_id, revision, verdict, summary, submitted_at)
                 VALUES (1, 1, 'approved', 'Ship it.', '{at}'),
                        (1, 2, 'approved', 'Approved again: the diff is unchanged since r1.',
                         '{at}'),
                        (1, 12, 'approved', 'Ship it again.', '{at}'),
                        (1, 13, 'approved', 'Approved again: the diff is unchanged since r12.',
                         '{at}');"
        ))
        .unwrap();

    let store = Store::open(&db).unwrap();

    let sent = store
        .sent_reviews(&ItemId::parse("app#1").unwrap())
        .unwrap();
    let sent: Vec<_> = sent.iter().map(|r| r.revision).collect();
    assert_eq!(sent, [1, 12]);
    let from = |revision: u32| -> Option<u32> {
        store
            .conn
            .query_row(
                "SELECT carried_from FROM reviews WHERE revision = ?1",
                [revision],
                |row| row.get(0),
            )
            .unwrap()
    };
    assert_eq!((from(2), from(13)), (Some(1), Some(12)));
}
