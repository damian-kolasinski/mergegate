use super::*;

#[test]
fn keeps_the_last_value_set() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    let mut store = Store::open(&db).unwrap();
    assert_eq!(store.setting("attachment-ttl").unwrap(), None);
    store.set_setting("attachment-ttl", "2d").unwrap();
    store.set_setting("attachment-ttl", "12h").unwrap();
    drop(store);
    let store = Store::open(&db).unwrap();
    assert_eq!(
        store.setting("attachment-ttl").unwrap().as_deref(),
        Some("12h")
    );
}
