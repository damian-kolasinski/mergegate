use super::*;
use crate::diff::{Content, LineKind};
use crate::fake::{FakeGit, FakeGithub, FakeStore, commit, rebased, submit_login};
use crate::item::{Submit, submit};
use crate::project::{Backend, Project, ProjectName, ProjectStore};
use crate::review::{NewReview, Verdict};

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
        .upstream("/src/app", "main", &commit(1));
    git.push("app", "login", &commit(2));
    let request = Submit {
        project: "app".into(),
        branch: "login".into(),
        head: commit(2),
        title: None,
        worktree: None,
        wake: None,
        guide: None,
    };
    submit(&mut store, &git, &FakeGithub::default(), request).unwrap();
    (store, git)
}

fn id(id: &str) -> ItemId {
    ItemId::parse(id).unwrap()
}

#[test]
fn reads_every_files_diff_in_path_order() {
    let store = submitted();
    let diff = item_diff(&store, &FakeGit::default(), &id("app#1"), Compare::Default)
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
    assert_eq!(
        item_diff(&submitted(), &git, &id("app#2"), Compare::Default).unwrap(),
        None
    );
    assert_eq!(
        item_diff(&submitted(), &git, &id("web#1"), Compare::Default).unwrap(),
        None
    );
}

#[test]
fn a_diff_missing_from_the_cache_is_an_error() {
    let mut store = submitted();
    store.forget_diffs();
    let err = item_diff(&store, &FakeGit::default(), &id("app#1"), Compare::Default).unwrap_err();
    assert_eq!(err.to_string(), "app#1 has no cached diff for README.md");
}

#[test]
fn an_abandoned_item_keeps_its_counts_but_not_its_diffs() {
    let (mut store, git) = submitted_with_git();
    let open = item_diff(&store, &git, &id("app#1"), Compare::Default)
        .unwrap()
        .unwrap();
    store.set_state(&id("app#1"), ItemState::Abandoned);
    store.forget_diffs();
    let read_at_submit = git.blobs_read().len();
    let diff = item_diff(&store, &git, &id("app#1"), Compare::Default)
        .unwrap()
        .unwrap();
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
    let open = item_diff(&store, &git, &id("app#1"), Compare::Default)
        .unwrap()
        .unwrap();
    store.set_state(&id("app#1"), ItemState::Merged);
    store.forget_diffs();
    let read_at_submit = git.blobs_read().len();
    let diff = item_diff(&store, &git, &id("app#1"), Compare::Default)
        .unwrap()
        .unwrap();
    assert_eq!(diff.files, open.files);
    assert!(git.blobs_read().len() > read_at_submit);
    assert!(store.diffs().is_empty());
}

#[test]
fn the_guide_orders_the_files_and_gives_reasons() {
    let mut store = submitted();
    let guide = "Adds login.\n\n## Files\n- `src/login.rs` — the new form\n";
    store.set_guide(&id("app#1"), 1, Some(guide)).unwrap();
    let diff = item_diff(&store, &FakeGit::default(), &id("app#1"), Compare::All)
        .unwrap()
        .unwrap();
    assert_eq!(diff.guide.as_deref(), Some(guide));
    let files: Vec<_> = diff
        .files
        .iter()
        .map(|f| (f.file.path.as_str(), f.reason.as_deref()))
        .collect();
    assert_eq!(
        files,
        [("src/login.rs", Some("the new form")), ("README.md", None)]
    );
}

fn request_changes(store: &mut FakeStore, revision: u32) {
    let review = NewReview {
        item: id("app#1"),
        revision,
        verdict: Verdict::ChangesRequested,
        summary: String::new(),
        moved: Vec::new(),
    };
    store
        .insert_review(review, ItemState::ChangesRequested)
        .unwrap();
}

fn changes(diff: &ItemDiff) -> Vec<(&str, Option<Change>)> {
    diff.files
        .iter()
        .map(|f| (f.file.path.as_str(), f.change))
        .collect()
}

#[test]
fn shows_what_changed_since_an_earlier_revision() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    submit_login(&mut store, &git, 4);
    submit_login(&mut store, &git, 5);

    let diff = item_diff(&store, &git, &id("app#1"), Compare::Since(1))
        .unwrap()
        .unwrap();
    assert_eq!(diff.since, Some(1));
    assert_eq!(diff.interdiffs, [1, 2]);
    assert_eq!(
        changes(&diff),
        [
            ("src/api.rs", Some(Change::Dropped)),
            ("src/db.rs", Some(Change::Unseparated)),
            ("src/lib.rs", Some(Change::Changed)),
            ("src/login.rs", Some(Change::Unchanged)),
            ("src/token.rs", Some(Change::Changed)),
        ]
    );
    let lib = &diff.files[2];
    let Some(Content::Text(hunks)) = &lib.content else {
        panic!("lib.rs has no text diff");
    };
    let lines: Vec<(LineKind, &str)> = hunks[0]
        .lines
        .iter()
        .map(|l| (l.kind, l.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            (LineKind::Removed, "lib login"),
            (LineKind::Added, "lib login2")
        ]
    );
}

#[test]
fn defaults_to_since_the_last_reviewed_revision() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    let compare = |store: &FakeStore| {
        let diff = item_diff(store, &git, &id("app#1"), Compare::Default)
            .unwrap()
            .unwrap();
        (diff.since, diff.last_reviewed)
    };
    assert_eq!(compare(&store), (None, None));
    request_changes(&mut store, 1);
    assert_eq!(compare(&store), (None, Some(1)));
    submit_login(&mut store, &git, 4);
    submit_login(&mut store, &git, 5);
    assert_eq!(compare(&store), (Some(1), Some(1)));

    let all = item_diff(&store, &git, &id("app#1"), Compare::All)
        .unwrap()
        .unwrap();
    assert_eq!(all.since, None);
    assert!(all.files.iter().all(|f| f.change.is_none()));
    assert_eq!(all.files.len(), 4);
}

#[test]
fn falls_back_to_all_changes_without_an_interdiff() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    submit_login(&mut store, &git, 4);
    let since = |store: &FakeStore, compare| {
        item_diff(store, &git, &id("app#1"), compare)
            .unwrap()
            .unwrap()
            .since
    };
    // Not earlier than the newest.
    assert_eq!(since(&store, Compare::Since(2)), None);
    assert_eq!(since(&store, Compare::Since(1)), Some(1));
    store.set_state(&id("app#1"), ItemState::Merged);
    assert_eq!(since(&store, Compare::Since(1)), None);
    store.set_state(&id("app#1"), ItemState::NeedsReview);
    store.forget_interdiffs();
    assert_eq!(since(&store, Compare::Since(1)), None);
}

#[test]
fn a_dropped_file_is_never_viewed() {
    let (mut store, git) = rebased();
    submit_login(&mut store, &git, 2);
    submit_login(&mut store, &git, 4);
    // A mark the newest revision can't have, as if it had.
    store
        .set_viewed(&id("app#1"), 2, "src/api.rs", true)
        .unwrap();
    store
        .set_viewed(&id("app#1"), 2, "src/lib.rs", true)
        .unwrap();
    let diff = item_diff(&store, &git, &id("app#1"), Compare::Since(1))
        .unwrap()
        .unwrap();
    let viewed: Vec<&str> = diff
        .files
        .iter()
        .filter(|f| f.viewed)
        .map(|f| f.file.path.as_str())
        .collect();
    assert_eq!(viewed, ["src/lib.rs"]);
}
