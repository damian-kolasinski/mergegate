use mergegate_core::item::{FileMode, ItemId};
use mergegate_core::judge::{Change, RequestFile};

use super::*;

fn file(path: &str, diff: Option<String>, tier: Tier) -> RequestFile {
    RequestFile {
        path: path.into(),
        old_path: None,
        change: Change::Modified,
        additions: diff.as_ref().map(|_| 1),
        deletions: diff.as_ref().map(|_| 0),
        tier,
        generated: false,
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        diff,
    }
}

fn request(files: Vec<RequestFile>) -> Request {
    Request {
        item: ItemId::parse("app#1").unwrap(),
        revision: 1,
        title: "Add login".into(),
        guide: Some("Adds a login form.\n".into()),
        files,
    }
}

fn diff(bytes: usize) -> Option<String> {
    Some(format!("@@ -1,1 +1,1 @@\n+{}\n", "x".repeat(bytes)))
}

#[test]
fn summarises_the_whole_change_in_each_part() {
    let request = request(vec![
        file("Cargo.lock", diff(10), Tier::Trusted),
        file("logo.png", None, Tier::Review),
        file("src/login.rs", diff(10), Tier::Review),
    ]);
    let parts = parts(&request).unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].files, [0, 2], "the binary file goes unjudged");
    let state = &parts[0].state;
    for line in [
        "Title: Add login",
        "Adds a login form.",
        "1. Cargo.lock (modified, +1 -0, marked safe to skip by a rule)",
        "2. logo.png (modified, binary)",
        "3. src/login.rs (modified, +1 -0)",
        "### File 3: src/login.rs\n@@ -1,1 +1,1 @@",
    ] {
        assert!(state.contains(line), "{line:?} in:\n{state}");
    }
}

#[test]
fn a_diff_that_names_a_secret_stays_on_the_machine() {
    let request = request(vec![
        file(
            ".env",
            Some("@@ -0,0 +1 @@\n+DATABASE_PASSWORD=hunter2\n".into()),
            Tier::Review,
        ),
        file("src/login.rs", diff(10), Tier::Review),
    ]);
    let parts = parts(&request).unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].files, [1], "the .env goes unjudged");
    let state = &parts[0].state;
    assert!(state.contains("1. .env (modified, +1 -0)"), "{state}");
    assert!(!state.contains("hunter2"), "{state}");
}

#[test]
fn splits_a_large_change_into_parts_of_whole_files() {
    // Three diffs of a third of the budget each (a token is 3 bytes), then
    // one over it.
    let third = BUDGET;
    let request = request(vec![
        file("a/one.rs", diff(third), Tier::Review),
        file("a/two.rs", diff(third), Tier::Review),
        file("b/three.rs", diff(third), Tier::Review),
        file("b/huge.rs", diff(BUDGET * 3), Tier::Review),
    ]);
    let parts = parts(&request).unwrap();
    let files: Vec<&[usize]> = parts.iter().map(|p| p.files.as_slice()).collect();
    assert_eq!(files, [&[0, 1][..], &[2][..]]);
    for part in &parts {
        assert!(tokens(&part.state) <= BUDGET);
        assert!(part.state.contains("4. b/huge.rs"), "each has the full map");
    }
}

#[test]
fn caps_the_files_a_part_asks_about() {
    let files = (0..MAX_FILES + 1)
        .map(|n| file(&format!("f{n}.rs"), diff(1), Tier::Review))
        .collect();
    let parts = parts(&request(files)).unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[1].files, [MAX_FILES]);
}

#[test]
fn names_what_the_modes_change_where_no_line_shows_it() {
    use FileMode::{Executable, Regular, Submodule, Symlink};
    let moded = |path, (old_mode, new_mode), diff| RequestFile {
        old_mode,
        new_mode,
        ..file(path, diff, Tier::Review)
    };
    let request = request(vec![
        moded("a.txt", (Some(Regular), Some(Symlink)), diff(1)),
        moded("dep", (None, Some(Submodule)), None),
        moded("evil", (None, Some(Symlink)), diff(1)),
        moded("run.sh", (Some(Regular), Some(Executable)), diff(1)),
    ]);
    let state = &parts(&request).unwrap()[0].state;
    for line in [
        "1. a.txt (modified, +1 -0, mode 100644 → 120000, a symlink: its line is its target)",
        "2. dep (modified, a submodule: its object is a commit of another repository)",
        "3. evil (modified, +1 -0, a symlink: its line is its target)",
        "4. run.sh (modified, +1 -0, mode 100644 → 100755)",
    ] {
        assert!(state.contains(line), "{line:?} in:\n{state}");
    }
}
