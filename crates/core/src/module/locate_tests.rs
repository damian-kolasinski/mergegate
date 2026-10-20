use super::*;
use crate::fake::FakeGit;
use crate::item::ChangedFile;
use crate::module::Placement;
use crate::triage::Triage;

fn stat(path: &str) -> FileStat {
    FileStat {
        file: ChangedFile {
            path: path.into(),
            old_path: None,
            old_blob: None,
            new_blob: None,
            old_mode: None,
            new_mode: None,
        },
        additions: None,
        deletions: None,
        generated: false,
        triage: Triage::default(),
        placement: None,
    }
}

#[test]
fn places_files_by_the_folders_at_the_base_and_the_head() {
    let git = FakeGit::default()
        .write(
            "b",
            None,
            "Base",
            &[
                ("Old/Old/A.swift", "a"),
                ("Old/OldTests/ATests.swift", "a"),
                ("crates/hub/Cargo.toml", "x"),
            ],
        )
        .write(
            "h",
            Some("b"),
            "Head",
            &[
                ("Old/Old/A.swift", ""),
                ("Old/OldTests/ATests.swift", ""),
                ("crates/hub/src/diff.rs", "x"),
            ],
        );
    let mut files = vec![
        stat("Old/Old/A.swift"),
        stat("crates/hub/src/diff.rs"),
        stat("README.md"),
    ];
    let project = ProjectName::parse("app").unwrap();

    locate(&git, &project, "b", "h", &mut files).unwrap();

    let placements: Vec<Option<Placement>> = files.into_iter().map(|f| f.placement).collect();
    assert_eq!(
        placements,
        [
            Some(Placement {
                module: "Old".into(),
                target: Some("Old".into()),
            }),
            Some(Placement {
                module: "crates/hub".into(),
                target: Some("src".into()),
            }),
            None,
        ]
    );
}
