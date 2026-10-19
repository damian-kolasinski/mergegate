use mergegate_core::item::{ItemBackend, ItemState, ItemStore, NewItem, NewRevision};
use mergegate_core::project::{Backend, Project, ProjectName, ProjectStore};

use super::*;

fn fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("hub.db")).unwrap();
    for project in ["app", "web"] {
        store
            .insert_project(&Project {
                name: ProjectName::parse(project).unwrap(),
                backend: Backend::Local,
                target: "main".into(),
                upstream: format!("/src/{project}").into(),
                github: None,
                bots: vec![],
            })
            .unwrap();
    }
    (dir, store)
}

fn item(store: &mut Store, project: &str, state: ItemState, local: &str) {
    let project = ProjectName::parse(project).unwrap();
    let branch = store.next_id(&project).unwrap().to_string();
    let item = store
        .insert_item(NewItem {
            project,
            branch,
            title: "Change".into(),
            state,
            backend: ItemBackend::Local,
            target: "main".into(),
            revision: NewRevision {
                head: "1".repeat(40),
                base: "0".repeat(40),
                files: vec![],
                guide: None,
                interdiffs: vec![],
                viewed: vec![],
                parent: None,
            },
            priority: None,
            note: None,
        })
        .unwrap();
    store
        .conn
        .execute(
            "UPDATE items SET closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', ?1, 'utc')
             WHERE number = ?2 AND project_id = (SELECT id FROM projects WHERE name = ?3)",
            params![local, item.id.number() as i64, item.id.project().as_str()],
        )
        .unwrap();
}

fn week(store: &Store, local: &str) -> WeeklyClosures {
    let utc: String = store
        .conn
        .query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', ?1, 'utc')",
            [local],
            |row| row.get(0),
        )
        .unwrap();
    store.weekly_closures_at(&utc).unwrap()
}

#[test]
fn counts_closures_by_local_day_and_repository_from_monday() {
    let (_dir, mut store) = fixture();
    for (project, state, at) in [
        ("app", ItemState::Merged, "2026-10-25 23:59:59.999"),
        ("app", ItemState::Merged, "2026-10-26 00:00:00"),
        ("app", ItemState::Abandoned, "2026-10-26 23:59:59.999"),
        ("app", ItemState::Merged, "2026-10-28 00:00:00"),
        ("app", ItemState::NeedsReview, "2026-10-28 12:00:00"),
        ("app", ItemState::Merged, "2026-11-02 00:00:00"),
        ("web", ItemState::Abandoned, "2026-10-27 12:00:00"),
        ("web", ItemState::Merged, "2026-11-01 23:59:59.999"),
    ] {
        item(&mut store, project, state, at);
    }
    let midweek = week(&store, "2026-10-28 18:00:00");
    assert_eq!(midweek.monday, "2026-10-26");
    assert_eq!(midweek.today, "2026-10-28");
    assert_eq!(midweek.repositories[0].project.as_str(), "app");
    assert_eq!(midweek.repositories[0].days, [2, 0, 1, 0, 0, 0, 0]);
    assert_eq!(midweek.repositories[1].project.as_str(), "web");
    assert_eq!(midweek.repositories[1].days, [0, 1, 0, 0, 0, 0, 0]);

    let sunday = week(&store, "2026-11-01 23:59:59.999");
    assert_eq!(sunday.monday, "2026-10-26");
    assert_eq!(sunday.repositories[1].days, [0, 1, 0, 0, 0, 0, 1]);
    let monday = week(&store, "2026-11-02 12:00:00");
    assert_eq!(monday.monday, "2026-11-02");
    assert_eq!(monday.repositories.len(), 1);
    assert_eq!(monday.repositories[0].days, [1, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn an_empty_week_keeps_its_calendar_and_handles_a_year_boundary() {
    let (_dir, store) = fixture();
    let week = week(&store, "2027-01-01 12:00:00");
    assert_eq!(week.monday, "2026-12-28");
    assert_eq!(week.today, "2027-01-01");
    assert!(week.repositories.is_empty());
}

#[test]
fn local_week_boundaries_hold_across_daylight_saving_changes() {
    for (monday, sunday, next) in [
        ("2026-03-23", "2026-03-29", "2026-03-30"),
        ("2026-10-19", "2026-10-25", "2026-10-26"),
    ] {
        let (_dir, mut store) = fixture();
        item(
            &mut store,
            "app",
            ItemState::Merged,
            &format!("{monday} 00:00:00"),
        );
        item(
            &mut store,
            "app",
            ItemState::Merged,
            &format!("{sunday} 23:59:59.999"),
        );
        item(
            &mut store,
            "app",
            ItemState::Merged,
            &format!("{next} 00:00:00"),
        );
        let week = week(&store, &format!("{sunday} 23:59:59.999"));
        assert_eq!(week.monday, monday);
        assert_eq!(week.repositories[0].days, [1, 0, 0, 0, 0, 0, 1]);
    }
}
