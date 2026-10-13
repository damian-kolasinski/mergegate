use mergegate_core::diff::compute;

use super::*;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("hub.db")).unwrap();
    (dir, store)
}

#[test]
fn round_trips_every_kind_of_diff_by_blob_pair() {
    let (_dir, mut store) = store();
    let old: String = (1..=20).map(|n| format!("{n}\n")).collect();
    let new = old.replace("5\n", "five\r\n").replace("20\n", "20");
    let text = compute(old.as_bytes(), new.as_bytes());
    let Content::Text(hunks) = &text.content else {
        panic!("{text:?}");
    };
    assert_eq!(hunks.len(), 2);
    let added = compute(b"", "\u{fffd}@@ x\n\\ y\n".as_bytes());
    let deleted = compute(b"gone\n", b"");
    let too_large = FileDiff {
        additions: Some(30_000),
        deletions: Some(1),
        content: Content::TooLarge,
    };
    let entries = [
        (Some("a"), Some("b"), text),
        (None, Some("c"), added),
        (Some("d"), None, deleted),
        (Some("e"), Some("f"), FileDiff::binary()),
        (Some("g"), Some("h"), too_large),
        (Some("a"), Some("a"), compute(b"same\n", b"same\n")),
    ];
    for (old, new, diff) in &entries {
        store.insert_diff(*old, *new, diff).unwrap();
    }
    for (old, new, diff) in &entries {
        assert_eq!(store.diff(*old, *new).unwrap().as_ref(), Some(diff));
    }
    assert_eq!(store.diff(Some("b"), Some("a")).unwrap(), None);
    assert_eq!(store.diff(None, Some("b")).unwrap(), None);
}

#[test]
fn replaces_an_entry_for_the_same_pair() {
    let (_dir, mut store) = store();
    store
        .insert_diff(Some("a"), None, &FileDiff::binary())
        .unwrap();
    let diff = compute(b"x\n", b"");
    store.insert_diff(Some("a"), None, &diff).unwrap();
    assert_eq!(store.diff(Some("a"), None).unwrap(), Some(diff));
}

#[test]
fn stores_hunks_as_a_unified_diff_body() {
    let (_dir, mut store) = store();
    let diff = compute(b"a\nb", b"a\nc\n");
    store.insert_diff(Some("a"), Some("b"), &diff).unwrap();
    let hunks: String = store
        .conn
        .query_row("SELECT hunks FROM diffs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        hunks,
        "@@ -1,2 +1,2 @@\n a\n-b\n\\ No newline at end of file\n+c\n"
    );
}

#[test]
fn rejects_a_malformed_body() {
    for body in [
        "x\n",
        "@@ -1 +1 @@\n",
        "@@ -1,1 +1,1 @@\n*a\n",
        "\\ No newline at end of file\n",
    ] {
        assert!(decode(body).is_err(), "{body:?}");
    }
}
