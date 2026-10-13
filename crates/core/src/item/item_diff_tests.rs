use super::*;
use crate::diff::{Content, LineKind};
use crate::fake::{FakeGit, FakeStore, commit};
use crate::item::{Submit, submit};
use crate::project::{Backend, Project, ProjectName, ProjectStore};

/// `app#1`: c2 adds `src/login.rs` and changes `README.md` on top of c1.
fn submitted() -> FakeStore {
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
    store
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

#[test]
fn reads_every_files_diff_in_path_order() {
    let store = submitted();
    let diff = item_diff(&store, &id("app#1")).unwrap().unwrap();
    assert_eq!(diff.item.title, "Add login");
    let paths: Vec<&str> = diff.files.iter().map(|f| f.file.path.as_str()).collect();
    assert_eq!(paths, ["README.md", "src/login.rs"]);
    let readme = &diff.files[0];
    assert_eq!(readme.file.old_blob, Some(format!("{}\n", commit(1))));
    let Content::Text(hunks) = &readme.diff.content else {
        panic!("not text: {:?}", readme.diff);
    };
    let kinds: Vec<LineKind> = hunks[0].lines.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, [LineKind::Context, LineKind::Added]);
}

#[test]
fn an_unknown_item_has_no_diff() {
    assert_eq!(item_diff(&submitted(), &id("app#2")).unwrap(), None);
    assert_eq!(item_diff(&submitted(), &id("web#1")).unwrap(), None);
}

#[test]
fn a_diff_missing_from_the_cache_is_an_error() {
    let mut store = submitted();
    store.forget_diffs();
    let err = item_diff(&store, &id("app#1")).unwrap_err();
    assert_eq!(err.to_string(), "app#1 has no cached diff for README.md");
}
