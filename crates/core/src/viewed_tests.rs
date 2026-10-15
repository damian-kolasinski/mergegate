use super::*;
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit, rebased, submit_login};
use crate::item::{Compare, Submit, item_diff, submit};
use crate::project::{Backend, Project, ProjectName, ProjectStore};

/// `app#1` r1 changes `README.md` and `src/login.rs`; r2 adds `src/token.rs`.
fn revised() -> FakeStore {
    let mut store = FakeStore::default();
    store
        .insert_project(&Project {
            name: ProjectName::parse("app").unwrap(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: "/src/app".into(),
            github: None,
            bots: Vec::new(),
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
        .commit(
            &commit(3),
            Some(&commit(2)),
            "Add tokens",
            &["src/token.rs"],
        )
        .upstream("/src/app", "main", &commit(1));
    for head in [commit(2), commit(3)] {
        git.push("app", "login", &head);
        let request = Submit {
            project: "app".into(),
            branch: "login".into(),
            head,
            title: None,
            worktree: None,
            wake: None,
            guide: None,
            priority: None,
            note: None,
        };
        submit(&mut store, &git, &FakeGithub::default(), request).unwrap();
    }
    store
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

fn marking(revision: u32, path: &str, viewed: bool) -> Mark {
    Mark {
        id: id("app#1"),
        revision,
        path: path.into(),
        viewed,
    }
}

fn viewed_in_diff(store: &FakeStore) -> Vec<String> {
    let diff = item_diff(store, &FakeGit::default(), &id("app#1"), Compare::Default)
        .unwrap()
        .unwrap();
    diff.files
        .into_iter()
        .filter(|f| f.viewed)
        .map(|f| f.file.path)
        .collect()
}

#[test]
fn marks_and_unmarks_a_file() {
    let mut store = revised();
    mark(&mut store, marking(2, "src/login.rs", true)).unwrap();
    mark(&mut store, marking(2, "README.md", true)).unwrap();
    assert_eq!(viewed_in_diff(&store), ["README.md", "src/login.rs"]);

    mark(&mut store, marking(2, "README.md", false)).unwrap();
    mark(&mut store, marking(2, "README.md", false)).unwrap();
    assert_eq!(viewed_in_diff(&store), ["src/login.rs"]);
}

#[test]
fn a_mark_moves_to_later_revisions_the_file_is_unchanged_in() {
    let (mut store, git) = rebased();
    for head in [2, 4, 5] {
        submit_login(&mut store, &git, head);
    }
    let viewed = |store: &FakeStore, revision| -> Vec<String> {
        store
            .viewed(&id("app#1"), revision)
            .unwrap()
            .into_iter()
            .collect()
    };
    mark(&mut store, marking(1, "src/login.rs", true)).unwrap();
    mark(&mut store, marking(1, "src/lib.rs", true)).unwrap();
    assert_eq!(viewed(&store, 1), ["src/lib.rs", "src/login.rs"]);
    assert_eq!(viewed(&store, 2), ["src/login.rs"]);
    assert_eq!(viewed(&store, 3), ["src/login.rs"]);

    mark(&mut store, marking(2, "src/lib.rs", true)).unwrap();
    assert_eq!(viewed(&store, 3), ["src/lib.rs", "src/login.rs"]);
    mark(&mut store, marking(1, "src/login.rs", false)).unwrap();
    assert_eq!(viewed(&store, 3), ["src/lib.rs"]);
    // Marking a later revision leaves earlier ones alone.
    mark(&mut store, marking(3, "src/token.rs", true)).unwrap();
    assert_eq!(viewed(&store, 2), ["src/lib.rs"]);
}

#[test]
fn a_mark_without_interdiffs_holds_for_its_revision_only() {
    let mut store = revised();
    store.forget_interdiffs();
    mark(&mut store, marking(1, "README.md", true)).unwrap();
    assert_eq!(viewed_in_diff(&store), Vec::<String>::new());
    let r1: Vec<String> = store.viewed(&id("app#1"), 1).unwrap().into_iter().collect();
    assert_eq!(r1, ["README.md"]);
}

#[test]
fn refuses_what_the_item_doesnt_have() {
    let mut store = revised();
    let err = |mark: Mark, store: &mut FakeStore| super::mark(store, mark).unwrap_err().to_string();
    let unknown = Mark {
        id: id("app#2"),
        ..marking(1, "README.md", true)
    };
    assert_eq!(err(unknown, &mut store), "no item app#2");
    assert_eq!(
        err(marking(3, "README.md", true), &mut store),
        "app#1 has no revision 3"
    );
    assert_eq!(
        err(marking(0, "README.md", true), &mut store),
        "app#1 has no revision 0"
    );
    // r1 didn't change it yet.
    assert_eq!(
        err(marking(1, "src/token.rs", true), &mut store),
        "app#1 r1 doesn't change src/token.rs"
    );
    assert_eq!(viewed_in_diff(&store), Vec::<String>::new());
}
