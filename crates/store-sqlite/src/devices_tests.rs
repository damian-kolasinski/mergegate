use super::*;

fn record(id: &str, name: &str, access: Option<Access>, asked_at: Option<u64>) -> Record {
    Record {
        id: DeviceId(id.into()),
        name: name.into(),
        owner: "someone@example.com".into(),
        access,
        asked_at,
    }
}

#[test]
fn keeps_decisions_by_node_across_renames_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    let mut store = Store::open(&db).unwrap();
    assert_eq!(store.device(&DeviceId("n1".into())).unwrap(), None);

    store
        .put_device(&record("n1", "worker-2", None, Some(1_700_000_000)))
        .unwrap();
    store
        .put_device(&record("n2", "ci", Some(Access::Deny), None))
        .unwrap();
    store
        .put_device(&record(
            "n1",
            "worker-renamed",
            Some(Access::Allow),
            Some(1_700_000_000),
        ))
        .unwrap();
    drop(store);

    let store = Store::open(&db).unwrap();
    assert_eq!(
        store.devices().unwrap(),
        [
            record("n2", "ci", Some(Access::Deny), None),
            record(
                "n1",
                "worker-renamed",
                Some(Access::Allow),
                Some(1_700_000_000)
            ),
        ]
    );
    assert_eq!(
        store.device(&DeviceId("n2".into())).unwrap(),
        Some(record("n2", "ci", Some(Access::Deny), None))
    );
}
