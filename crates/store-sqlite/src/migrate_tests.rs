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
fn closing_events_keep_the_cursors_reviews_were_sent_with() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    drop(open_with(&db, &MIGRATIONS[..8]).unwrap());
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO projects VALUES (1, 'app', 'local', 'main', '/src/app');
         INSERT INTO items VALUES (1, 1, 1, 'login', 'Add login', 'changes_requested',
                                   'local', 'main', 't', 0);
         INSERT INTO revisions VALUES (1, 1, 'c2', 'c1', 1, 1, 0, 't');
         INSERT INTO revisions VALUES (1, 2, 'c3', 'c1', 1, 1, 0, 't');
         INSERT INTO reviews VALUES (7, 1, 2, 'changes_requested', 'Fix it', 't');
         INSERT INTO events (cursor, item_id, type, review_id)
             VALUES (5, 1, 'review.submitted', 7);
         INSERT INTO events (cursor, item_id, type, review_id)
             VALUES (9, 1, 'review.submitted', 7);
         DELETE FROM events WHERE cursor = 9;
         INSERT INTO comments (item_id, revision, review_id, path, side, start_line,
                               line, body, quote, created_at)
             VALUES (1, 2, 7, 'a.rs', 'new', 1, 1, 'No', 'x', 't');",
    )
    .unwrap();
    drop(conn);

    drop(Store::open(&db).unwrap());
    let conn = Connection::open(&db).unwrap();
    let row: (i64, i64, String, i64) = conn
        .query_row(
            "SELECT cursor, revision, source, review_id FROM events",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(row, (5, 2, "operator".into(), 7));
    // The sequence moved with the table, so a new event can't reuse a cursor.
    conn.execute(
        "INSERT INTO events (item_id, type, revision, source)
         VALUES (1, 'item.abandoned', 2, 'agent')",
        [],
    )
    .unwrap();
    let cursor: i64 = conn
        .query_row("SELECT max(cursor) FROM events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(cursor, 10);
    let verdict: String = conn
        .query_row(
            "SELECT r.verdict FROM reviews r JOIN comments c ON c.review_id = r.id",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(verdict, "changes_requested");
}
