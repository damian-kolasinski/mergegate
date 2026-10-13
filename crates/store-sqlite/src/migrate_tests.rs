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
