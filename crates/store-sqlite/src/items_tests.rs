use mergegate_core::feedback::{FeedbackStore, Notice, NoticeKind, Source};
use mergegate_core::github::{GithubRepo, Pull};
use mergegate_core::item::{ChangedFile, FileMode, FileStat};
use mergegate_core::project::{Backend, Project, ProjectStore};

use super::*;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    for name in ["app", "web"] {
        let project = Project {
            name: ProjectName::parse(name).unwrap(),
            backend: Backend::Local,
            target: "main".into(),
            upstream: format!("/src/{name}").into(),
            github: None,
            bots: Vec::new(),
        };
        store.insert_project(&project).unwrap();
    }
    (dir, store)
}

fn name(project: &str) -> ProjectName {
    ProjectName::parse(project).unwrap()
}

fn revision(head: char) -> NewRevision {
    NewRevision {
        head: head.to_string().repeat(40),
        base: "0".repeat(40),
        files: vec![
            FileStat {
                file: ChangedFile {
                    path: "src/new.rs".into(),
                    old_path: Some("src/old.rs".into()),
                    old_blob: Some("1".repeat(40)),
                    new_blob: Some(head.to_string().repeat(40)),
                    old_mode: Some(FileMode::Regular),
                    new_mode: Some(FileMode::Executable),
                },
                additions: Some(5),
                deletions: Some(2),
                generated: false,
                triage: Default::default(),
            },
            FileStat {
                file: ChangedFile {
                    path: "logo.png".into(),
                    old_path: None,
                    old_blob: None,
                    new_blob: Some("2".repeat(40)),
                    old_mode: None,
                    new_mode: Some(FileMode::Regular),
                },
                additions: None,
                deletions: None,
                generated: false,
                triage: Default::default(),
            },
        ],
        guide: None,
        interdiffs: Vec::new(),
        viewed: Vec::new(),
    }
}

fn new_item(project: &str, branch: &str) -> NewItem {
    NewItem {
        project: name(project),
        branch: branch.into(),
        title: format!("Work on {branch}"),
        state: ItemState::NeedsReview,
        backend: ItemBackend::Local,
        target: "main".into(),
        revision: revision('a'),
        priority: None,
        note: None,
    }
}

#[test]
fn numbers_items_per_project_and_lists_open_ones_oldest_first() {
    let (_dir, mut store) = store();
    assert_eq!(store.next_id(&name("app")).unwrap().to_string(), "app#1");
    let a = store.insert_item(new_item("app", "a")).unwrap();
    let b = store.insert_item(new_item("web", "b")).unwrap();
    assert_eq!(store.next_id(&name("app")).unwrap().to_string(), "app#2");
    let c = store.insert_item(new_item("app", "c")).unwrap();

    assert_eq!(a.id.to_string(), "app#1");
    assert_eq!(b.id.to_string(), "web#1");
    assert_eq!(c.id.to_string(), "app#2");
    assert_eq!(a.title, "Work on a");
    assert_eq!(
        (a.state, &a.backend, a.target.as_str()),
        (ItemState::NeedsReview, &ItemBackend::Local, "main")
    );
    assert_eq!(a.revision.number, 1);
    assert_eq!(a.revision.head, "a".repeat(40));
    assert_eq!(
        a.revision.stat,
        DiffStat {
            files: 2,
            additions: 5,
            deletions: 2,
            review_files: 2,
        }
    );
    assert_eq!(a.created_at, a.revision.submitted_at);
    assert!(a.created_at.ends_with('Z'), "{}", a.created_at);

    assert_eq!(store.open_items(None).unwrap(), [a.clone(), b, c.clone()]);
    assert_eq!(
        store.open_items(Some(&name("app"))).unwrap(),
        [a.clone(), c]
    );
    assert_eq!(store.open_item(&name("app"), "a").unwrap(), Some(a));
    assert_eq!(store.open_item(&name("web"), "a").unwrap(), None);
}

#[test]
fn keeps_a_github_items_pull_request() {
    let (_dir, mut store) = store();
    let mut web = store.project(&name("web")).unwrap().unwrap();
    web.backend = Backend::Github;
    web.github = GithubRepo::parse("acme/web");
    store.update_project(&web).unwrap();
    let backend = ItemBackend::Github {
        repo: GithubRepo::parse("acme/web").unwrap(),
        pull: Pull {
            number: 12,
            url: "https://github.com/acme/web/pull/12".into(),
        },
    };
    let item = store
        .insert_item(NewItem {
            backend: backend.clone(),
            ..new_item("web", "a")
        })
        .unwrap();
    assert_eq!(item.backend, backend);
    assert_eq!(store.item(&item.id).unwrap(), Some(item));
}

#[test]
fn finds_an_item_by_its_pull_request_the_open_one_first() {
    let (_dir, mut store) = store();
    let on = |repo: &str, number| ItemBackend::Github {
        repo: GithubRepo::parse(repo).unwrap(),
        pull: Pull {
            number,
            url: format!("https://github.com/{repo}/pull/{number}"),
        },
    };
    let insert = |store: &mut Store, branch, backend| {
        store
            .insert_item(NewItem {
                backend,
                ..new_item("web", branch)
            })
            .unwrap()
    };
    // The project moved repositories, and both have a pull request #7.
    let old = insert(&mut store, "a", on("acme/old", 7));
    let new = insert(&mut store, "b", on("acme/web", 7));
    insert(&mut store, "c", on("acme/web", 8));
    let found = |store: &Store, number| {
        store
            .item_with_pull(&name("web"), number)
            .unwrap()
            .map(|item| item.id.to_string())
    };
    assert_eq!(found(&store, 7), Some(new.id.to_string()));
    assert_eq!(found(&store, 9), None);
    assert_eq!(store.item_with_pull(&name("app"), 7).unwrap(), None);

    store
        .conn
        .execute("UPDATE items SET state = 'merged' WHERE branch = 'b'", [])
        .unwrap();
    assert_eq!(found(&store, 7), Some(old.id.to_string()));
    store
        .conn
        .execute("UPDATE items SET state = 'merged' WHERE branch = 'a'", [])
        .unwrap();
    assert_eq!(found(&store, 7), Some(new.id.to_string()));
}

#[test]
fn reads_any_item_by_id() {
    let (_dir, mut store) = store();
    let a = store.insert_item(new_item("app", "a")).unwrap();
    store
        .conn
        .execute("UPDATE items SET state = 'merged'", [])
        .unwrap();
    let merged = store.item(&a.id).unwrap().unwrap();
    assert_eq!(merged.state, ItemState::Merged);
    assert_eq!(merged.revision, a.revision);
    assert_eq!(store.item(&ItemId::new(name("app"), 2)).unwrap(), None);
    assert_eq!(store.item(&ItemId::new(name("web"), 1)).unwrap(), None);
}

#[test]
fn lists_a_projects_last_merged_items_newest_first() {
    let (_dir, mut store) = store();
    let ids: Vec<_> = ["a", "b", "c", "d"]
        .into_iter()
        .map(|branch| store.insert_item(new_item("app", branch)).unwrap().id)
        .collect();
    store.insert_item(new_item("web", "a")).unwrap();
    store
        .conn
        .execute("UPDATE items SET state = 'merged' WHERE number != 3", [])
        .unwrap();
    let merged = |limit| -> Vec<_> {
        store
            .merged_items(&name("app"), limit)
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect()
    };
    assert_eq!(merged(20), [ids[3].clone(), ids[1].clone(), ids[0].clone()]);
    assert_eq!(merged(1), [ids[3].clone()]);
}

#[test]
fn lists_every_projects_closed_items_oldest_first() {
    let (_dir, mut store) = store();
    let a = store.insert_item(new_item("app", "a")).unwrap().id;
    store.insert_item(new_item("app", "b")).unwrap();
    let web = store.insert_item(new_item("web", "a")).unwrap().id;
    store
        .conn
        .execute("UPDATE items SET state = 'merged' WHERE branch = 'a'", [])
        .unwrap();
    store
        .conn
        .execute("UPDATE items SET state = 'abandoned' WHERE id = 1", [])
        .unwrap();
    let closed: Vec<_> = store
        .closed_items()
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(closed, [a, web]);
}

#[test]
fn lists_items_closed_since_midnight_latest_first() {
    let (_dir, mut store) = store();
    let ids: Vec<_> = ["a", "b", "c", "d"]
        .into_iter()
        .map(|branch| store.insert_item(new_item("app", branch)).unwrap().id)
        .collect();
    let close = |number: u32, state: &str, at: &str| {
        store
            .conn
            .execute(
                &format!("UPDATE items SET state = ?1, closed_at = {at} WHERE number = ?2"),
                params![state, number],
            )
            .unwrap();
    };
    close(
        1,
        "merged",
        "strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 second')",
    );
    close(2, "abandoned", "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')");
    // Before the operator's midnight, whatever their timezone.
    close(
        4,
        "merged",
        "strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 days')",
    );
    let closed: Vec<_> = store
        .closed_today()
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(closed, [ids[1].clone(), ids[0].clone()]);
}

#[test]
fn a_closed_item_was_last_updated_as_it_closed() {
    let (_dir, mut store) = store();
    let open = store.insert_item(new_item("app", "a")).unwrap();
    assert_eq!(open.closed_at, None);
    assert_eq!(open.updated_at(), open.revision.submitted_at);
    // A close the same millisecond as the submit couldn't tell them apart.
    store
        .conn
        .execute(
            "UPDATE revisions SET submitted_at = '2026-01-01T00:00:00.000Z'",
            [],
        )
        .unwrap();
    let notice = Notice {
        revision: 1,
        source: Source::Operator,
        kind: NoticeKind::Abandoned { reason: None },
    };
    store
        .notify(&open.id, ItemState::Abandoned, notice)
        .unwrap();
    let closed = store.item(&open.id).unwrap().unwrap();
    let closed_at = closed.closed_at.clone().unwrap();
    assert!(closed_at.ends_with('Z'), "{closed_at}");
    assert_eq!(closed.updated_at(), closed_at);
    assert!(
        closed.updated_at() > closed.revision.submitted_at.as_str(),
        "{closed:?}"
    );
    assert_eq!(store.closed_today().unwrap(), [closed]);
}

#[test]
fn adds_revisions_with_their_files() {
    let (_dir, mut store) = store();
    let first = store.insert_item(new_item("app", "a")).unwrap();
    let item = store
        .add_revision(&first.id, "Renamed", ItemState::NeedsReview, revision('b'))
        .unwrap();
    assert_eq!(item.id, first.id);
    assert_eq!(item.title, "Renamed");
    assert_eq!(item.revision.number, 2);
    assert_eq!(item.revision.head, "b".repeat(40));
    assert_eq!(item.created_at, first.created_at);
    assert_eq!(store.open_items(None).unwrap(), std::slice::from_ref(&item));

    let files: Vec<(i64, String, Option<String>, Option<i64>)> = store
        .conn
        .prepare("SELECT revision, path, old_path, additions FROM revision_files ORDER BY revision, position")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(files.len(), 4);
    assert_eq!(
        files[2],
        (2, "src/new.rs".into(), Some("src/old.rs".into()), Some(5))
    );
    assert_eq!(files[3], (2, "logo.png".into(), None, None));

    let changed = |revision| store.revision_files(&item.id, revision).unwrap();
    assert_eq!(changed(1), revision('a').files);
    assert_eq!(changed(2), revision('b').files);
    assert!(changed(3).is_empty());

    let revisions = store.revisions(&item.id).unwrap();
    let heads: Vec<(u32, String)> = revisions
        .iter()
        .map(|r| (r.number, r.head.clone()))
        .collect();
    assert_eq!(heads, [(1, "a".repeat(40)), (2, "b".repeat(40))]);
    assert_eq!(revisions[1], item.revision);
    assert!(
        store
            .revisions(&ItemId::parse("app#9").unwrap())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn allows_one_open_item_per_branch() {
    let (_dir, mut store) = store();
    let first = store.insert_item(new_item("app", "a")).unwrap();
    assert!(store.insert_item(new_item("app", "a")).is_err());

    store
        .conn
        .execute("UPDATE items SET state = 'merged'", [])
        .unwrap();
    assert!(store.open_items(None).unwrap().is_empty());
    let next = store.insert_item(new_item("app", "a")).unwrap();
    assert_eq!(next.id.number(), first.id.number() + 1);
}

#[test]
fn keeps_each_revisions_guide() {
    let (_dir, mut store) = store();
    let mut item = new_item("app", "a");
    item.revision.guide = Some("## Files\n- `src/new.rs`\n".into());
    let id = store.insert_item(item).unwrap().id;
    store
        .add_revision(&id, "Work on a", ItemState::NeedsReview, revision('b'))
        .unwrap();

    assert_eq!(
        store.guide(&id, 1).unwrap().as_deref(),
        Some("## Files\n- `src/new.rs`\n")
    );
    assert_eq!(store.guide(&id, 2).unwrap(), None);
    store.set_guide(&id, 2, Some("r2's")).unwrap();
    store.set_guide(&id, 1, None).unwrap();
    assert_eq!(store.guide(&id, 1).unwrap(), None);
    assert_eq!(store.guide(&id, 2).unwrap().as_deref(), Some("r2's"));
    assert_eq!(store.guide(&id, 3).unwrap(), None);
}

#[test]
fn retitles_one_item() {
    let (_dir, mut store) = store();
    let item = store.insert_item(new_item("app", "a")).unwrap();
    let other = store.insert_item(new_item("web", "a")).unwrap();
    store.set_title(&item.id, "Login page").unwrap();
    assert_eq!(store.item(&item.id).unwrap().unwrap().title, "Login page");
    assert_eq!(store.item(&other.id).unwrap().unwrap().title, other.title);
}

#[test]
fn keeps_an_items_priority_and_note() {
    let (_dir, mut store) = store();
    let item = store
        .insert_item(NewItem {
            priority: Some(Priority::P1),
            note: Some("blocks the release".into()),
            ..new_item("app", "a")
        })
        .unwrap();
    assert_eq!(
        (item.priority, item.note.as_deref()),
        (Some(Priority::P1), Some("blocks the release"))
    );
    let other = store.insert_item(new_item("web", "a")).unwrap();
    assert_eq!((other.priority, other.note.as_deref()), (None, None));

    store
        .set_priority(&item.id, Some(Priority::P0), None)
        .unwrap();
    let item = store.item(&item.id).unwrap().unwrap();
    assert_eq!((item.priority, item.note), (Some(Priority::P0), None));
    store.set_priority(&item.id, None, Some("later")).unwrap();
    let item = store.item(&item.id).unwrap().unwrap();
    assert_eq!((item.priority, item.note.as_deref()), (None, Some("later")));
    assert_eq!(store.item(&other.id).unwrap().unwrap().note, None);
}
