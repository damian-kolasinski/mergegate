use super::unpaired;
use crate::fake::{FakeGit, FakeStore, commit, rebased, submit_login};
use crate::interdiff::{Change, InterdiffStore};
use crate::item::{ChangedFile, FileMode, FileStat, ItemId};
use crate::triage::Triage;
use crate::viewed::ViewedStore;

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn changes(store: &FakeStore, from: u32, to: u32) -> Vec<(String, Change)> {
    store
        .interdiff(&id(), from, to)
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|f| (f.stat.file.path, f.change))
        .collect()
}

fn owned(changes: &[(&str, Change)]) -> Vec<(String, Change)> {
    changes.iter().map(|(p, c)| ((*p).into(), *c)).collect()
}

#[test]
fn replays_the_earlier_revision_onto_the_moved_base() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    submit_login(&mut store, &git, 4);

    // README.md moved with the target alone, so it isn't there at all.
    assert_eq!(
        changes(&store, 1, 2),
        owned(&[
            ("src/api.rs", Change::Dropped),
            ("src/db.rs", Change::Unseparated),
            ("src/lib.rs", Change::Changed),
            ("src/login.rs", Change::Unchanged),
            ("src/token.rs", Change::Changed),
        ])
    );
    assert_eq!(store.interdiffs_to(&id(), 2).unwrap(), [1]);
}

#[test]
fn files_carry_the_diff_they_show() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    submit_login(&mut store, &git, 4);

    let files = store.interdiff(&id(), 1, 2).unwrap().unwrap();
    let blobs = |path: &str| {
        let file = &files
            .iter()
            .find(|f| f.stat.file.path == path)
            .unwrap()
            .stat;
        (file.file.old_blob.clone(), file.file.new_blob.clone())
    };
    let some = |blob: &str| Some(blob.to_owned());
    // Changed and dropped: from the replayed r1 to r2.
    assert_eq!(blobs("src/lib.rs"), (some("lib login"), some("lib login2")));
    assert_eq!(blobs("src/api.rs"), (some("api"), None));
    // Unseparated and unchanged: r2's own diff.
    assert_eq!(blobs("src/db.rs"), (some("db 2"), some("db r2")));
    assert_eq!(blobs("src/login.rs"), (None, some("login")));
    for (old, new) in [
        (some("lib login"), some("lib login2")),
        (some("api"), None),
        (None, some("token")),
    ] {
        assert!(store.diffs().contains_key(&(old, new)));
    }
}

#[test]
fn pins_each_replayed_tree_until_the_item_closes() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    submit_login(&mut store, &git, 4);
    submit_login(&mut store, &git, 5);

    let pins = git.pins();
    let trees = git.trees();
    assert!(trees.contains_key(&pins["refs/mergegate/items/1/r1-r2"]));
    assert!(trees.contains_key(&pins["refs/mergegate/items/1/r1-r3"]));
    // r2 and r3 share a base: nothing to replay.
    assert!(!pins.contains_key("refs/mergegate/items/1/r2-r3"));
    assert_eq!(pins["refs/mergegate/items/1/r3"], commit(5));
}

#[test]
fn an_unmoved_base_diffs_the_heads() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    submit_login(&mut store, &git, 4);
    submit_login(&mut store, &git, 5);

    assert_eq!(
        changes(&store, 2, 3),
        owned(&[
            ("src/db.rs", Change::Unchanged),
            ("src/lib.rs", Change::Unchanged),
            ("src/login.rs", Change::Unchanged),
            ("src/token.rs", Change::Changed),
        ])
    );
    assert_eq!(store.interdiffs_to(&id(), 3).unwrap(), [1, 2]);
    assert_eq!(
        changes(&store, 1, 3),
        owned(&[
            ("src/api.rs", Change::Dropped),
            ("src/db.rs", Change::Unseparated),
            ("src/lib.rs", Change::Changed),
            ("src/login.rs", Change::Unchanged),
            ("src/token.rs", Change::Changed),
        ])
    );
}

#[test]
fn carries_marks_from_the_latest_revision_a_file_is_unchanged_since() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    for path in ["src/login.rs", "src/lib.rs", "src/db.rs"] {
        store.set_viewed(&id(), 1, path, true).unwrap();
    }
    submit_login(&mut store, &git, 4);
    let viewed = |store: &FakeStore, revision| -> Vec<String> {
        store.viewed(&id(), revision).unwrap().into_iter().collect()
    };
    assert_eq!(viewed(&store, 2), ["src/login.rs"]);

    // r3 leaves lib.rs as r2 had it, which wasn't viewed: r1's mark doesn't
    // come back.
    store.set_viewed(&id(), 2, "src/token.rs", true).unwrap();
    store.set_viewed(&id(), 2, "src/login.rs", false).unwrap();
    submit_login(&mut store, &git, 5);
    assert_eq!(viewed(&store, 3), Vec::<String>::new());
}

#[test]
fn a_mode_change_alone_drops_a_viewed_mark() {
    let mut store = FakeStore::with_item(0);
    let git = FakeGit::default()
        .write(&commit(1), None, "Initial", &[("run.sh", "echo")])
        .write(
            &commit(2),
            Some(&commit(1)),
            "Add login",
            &[("run.sh", "echo login"), ("src/login.rs", "login")],
        )
        .chmod(
            &commit(3),
            Some(&commit(2)),
            "Make it runnable",
            &[("run.sh", FileMode::Executable)],
        )
        .upstream("/src/app", "main", &commit(1));
    submit_login(&mut store, &git, 2);
    for path in ["run.sh", "src/login.rs"] {
        store.set_viewed(&id(), 1, path, true).unwrap();
    }
    submit_login(&mut store, &git, 3);

    assert_eq!(
        changes(&store, 1, 2),
        owned(&[
            ("run.sh", Change::Changed),
            ("src/login.rs", Change::Unchanged),
        ])
    );
    let files = store.interdiff(&id(), 1, 2).unwrap().unwrap();
    let run = &files[0].stat.file;
    assert_eq!(
        (run.old_mode, run.new_mode),
        (Some(FileMode::Regular), Some(FileMode::Executable))
    );
    let viewed: Vec<String> = store.viewed(&id(), 2).unwrap().into_iter().collect();
    assert_eq!(viewed, ["src/login.rs"]);
}

fn moved(path: &str, from: Option<&str>, old: Option<&str>, new: Option<&str>) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        old_path: from.map(Into::into),
        old_blob: old.map(Into::into),
        new_blob: new.map(Into::into),
        old_mode: old.map(|_| FileMode::Regular),
        new_mode: new.map(|_| FileMode::Regular),
    }
}

fn own(files: &[ChangedFile]) -> Vec<FileStat> {
    files
        .iter()
        .map(|file| FileStat {
            file: file.clone(),
            additions: None,
            deletions: None,
            generated: false,
            triage: Triage::default(),
        })
        .collect()
}

#[test]
fn a_rename_away_from_an_own_file_splits() {
    // r1 rewrote a.txt, so against the base r2 deletes it and adds b.txt.
    let own = own(&[
        moved("a.txt", None, Some("base"), None),
        moved("b.txt", None, None, Some("r1")),
    ]);
    assert_eq!(
        unpaired(moved("b.txt", Some("a.txt"), Some("r1"), Some("r1")), &own),
        [
            moved("a.txt", None, Some("r1"), None),
            moved("b.txt", None, None, Some("r1")),
        ]
    );
}

#[test]
fn a_split_rename_keeps_each_sides_mode() {
    let own = own(&[
        moved("a.txt", None, Some("base"), None),
        moved("b.sh", None, None, Some("r1")),
    ]);
    let rename = ChangedFile {
        old_mode: Some(FileMode::Symlink),
        new_mode: Some(FileMode::Executable),
        ..moved("b.sh", Some("a.txt"), Some("r1"), Some("r1"))
    };
    let modes: Vec<_> = unpaired(rename, &own)
        .into_iter()
        .map(|f| (f.path, f.old_mode, f.new_mode))
        .collect();
    assert_eq!(
        modes,
        [
            ("a.txt".into(), Some(FileMode::Symlink), None),
            ("b.sh".into(), None, Some(FileMode::Executable)),
        ]
    );
}

#[test]
fn a_rename_the_own_diff_shares_stays_whole() {
    let own = own(&[moved("b.txt", Some("a.txt"), Some("base"), Some("r2"))]);
    let rename = moved("b.txt", Some("a.txt"), Some("r1"), Some("r2"));
    assert_eq!(unpaired(rename.clone(), &own), [rename]);
}

#[test]
fn a_rename_of_a_file_the_item_added_stays_whole() {
    let own = own(&[moved("b.txt", None, None, Some("r1"))]);
    let rename = moved("b.txt", Some("a.txt"), Some("r1"), Some("r1"));
    assert_eq!(unpaired(rename.clone(), &own), [rename]);
}
