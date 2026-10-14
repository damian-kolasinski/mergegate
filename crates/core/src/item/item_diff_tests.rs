use super::*;
use crate::diff::{Content, LineKind};
use crate::fake::{FakeGit, FakeStore, commit};
use crate::item::{Submit, submit};
use crate::project::{Backend, Project, ProjectName, ProjectStore};

/// `app#1`: c2 adds `src/login.rs` and changes `README.md` on top of c1.
fn submitted() -> FakeStore {
    submitted_with_git().0
}

fn submitted_with_git() -> (FakeStore, FakeGit) {
    let mut store = FakeStore::default();
    store
        .insert_project(&Project {
            name: ProjectName::parse("app").unwrap(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
        })
        .unwrap();
    let git = FakeGit::default()
        .commit(&commit(1), None, "Initial", &["README.md"])
        .commit(
            &commit(2),
            Some(&commit(1)),
            "Add login",
            &["src/login.rs", "README.md"],
        )
        .upstream("/src/app", "main", &commit(1));
    git.push("app", "login", &commit(2));
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(2),
        title: None,
    };
    submit(&mut store, &git, request).unwrap();
    (store, git)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

#[test]
fn reads_every_files_diff_in_path_order() {
    let store = submitted();
    let diff = item_diff(&store, &FakeGit::default(), &id("app#1"))
        .unwrap()
        .unwrap();
    assert_eq!(diff.item.title, "Add login");
    let paths: Vec<&str> = diff.files.iter().map(|f| f.file.path.as_str()).collect();
    assert_eq!(paths, ["README.md", "src/login.rs"]);
    let readme = &diff.files[0];
    assert_eq!(readme.file.old_blob, Some(format!("{}\n", commit(1))));
    let Some(Content::Text(hunks)) = &readme.content else {
        panic!("not text: {:?}", readme.content);
    };
    let kinds: Vec<LineKind> = hunks[0].lines.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, [LineKind::Context, LineKind::Added]);
}

#[test]
fn an_unknown_item_has_no_diff() {
    let git = FakeGit::default();
    assert_eq!(item_diff(&submitted(), &git, &id("app#2")).unwrap(), None);
    assert_eq!(item_diff(&submitted(), &git, &id("web#1")).unwrap(), None);
}

#[test]
fn a_diff_missing_from_the_cache_is_an_error() {
    let mut store = submitted();
    store.forget_diffs();
    let err = item_diff(&store, &FakeGit::default(), &id("app#1")).unwrap_err();
    assert_eq!(err.to_string(), "app#1 has no cached diff for README.md");
}

#[test]
fn an_abandoned_item_keeps_its_counts_but_not_its_diffs() {
    let (mut store, git) = submitted_with_git();
    let open = item_diff(&store, &git, &id("app#1")).unwrap().unwrap();
    store.set_state(&id("app#1"), ItemState::Abandoned);
    store.forget_diffs();
    let read_at_submit = git.blobs_read().len();
    let diff = item_diff(&store, &git, &id("app#1")).unwrap().unwrap();
    for (file, was) in diff.files.iter().zip(&open.files) {
        assert_eq!(file.content, None);
        assert_eq!(
            (file.additions, file.deletions),
            (was.additions, was.deletions)
        );
    }
    assert_eq!(git.blobs_read().len(), read_at_submit);
}

#[test]
fn a_merged_item_is_diffed_again_and_not_cached() {
    let (mut store, git) = submitted_with_git();
    let open = item_diff(&store, &git, &id("app#1")).unwrap().unwrap();
    store.set_state(&id("app#1"), ItemState::Merged);
    store.forget_diffs();
    let read_at_submit = git.blobs_read().len();
    let diff = item_diff(&store, &git, &id("app#1")).unwrap().unwrap();
    assert_eq!(diff.files, open.files);
    assert!(git.blobs_read().len() > read_at_submit);
    assert!(store.diffs().is_empty());
}
