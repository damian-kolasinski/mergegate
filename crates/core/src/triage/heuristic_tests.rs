use super::*;
use crate::diff::{Hunk, Line};
use crate::item::{ChangedFile, FileMode};

/// A regular file modified at `path`.
fn changed(path: &str) -> ChangedFile {
    moded(path, Some(FileMode::Regular), Some(FileMode::Regular))
}

fn moded(path: &str, old: Option<FileMode>, new: Option<FileMode>) -> ChangedFile {
    ChangedFile {
        path: path.into(),
        old_path: None,
        old_blob: old.map(|_| "old".into()),
        new_blob: new.map(|_| "new".into()),
        old_mode: old,
        new_mode: new,
    }
}

fn text(lines: &[(LineKind, &str)]) -> Content {
    Content::Text(vec![Hunk {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        lines: lines
            .iter()
            .map(|&(kind, text)| Line {
                kind,
                text: text.into(),
                newline: true,
            })
            .collect(),
    }])
}

#[test]
fn hints_by_path() {
    let body = text(&[(LineKind::Added, "    let x = 1;")]);
    let cases: &[(&str, Option<Hint>)] = &[
        ("Cargo.lock", Some(Hint::Lockfile)),
        ("ui/pnpm-lock.yaml", Some(Hint::Lockfile)),
        ("vendor/thing.lock", Some(Hint::Lockfile)),
        ("crates/core/src/item_tests.rs", Some(Hint::Tests)),
        ("crates/cli/tests/rules.rs", Some(Hint::Tests)),
        ("ui/tests/triage.spec.ts", Some(Hint::Tests)),
        ("src/Login.test.tsx", Some(Hint::Tests)),
        ("App/LoginViewTests.swift", Some(Hint::Tests)),
        ("pkg/auth_test.go", Some(Hint::Tests)),
        ("test_auth.py", Some(Hint::Tests)),
        ("README.md", Some(Hint::Docs)),
        ("docs/setup.html", Some(Hint::Docs)),
        ("NOTES.txt", Some(Hint::Docs)),
        ("CMakeLists.txt", Some(Hint::Config)),
        ("api/requirements.txt", Some(Hint::Config)),
        ("migrations/0008_judge.sql", Some(Hint::Migration)),
        ("db/migrate/20260101_add.rb", Some(Hint::Migration)),
        ("Cargo.toml", Some(Hint::Config)),
        ("crates/hub/Cargo.toml", Some(Hint::Config)),
        (".github/workflows/ci.yml", Some(Hint::Config)),
        (".env.example", Some(Hint::Config)),
        ("src/login.rs", None),
        ("src/contest.rs", None),
    ];
    for &(path, want) in cases {
        assert_eq!(
            hint(&changed(path), false, Some(1), Some(0), Some(&body)),
            want,
            "{path}"
        );
    }
}

#[test]
fn generated_files_sink_unless_they_are_lockfiles() {
    assert_eq!(
        hint(&changed("src/api.ts"), true, Some(1), Some(0), None),
        Some(Hint::Generated)
    );
    assert_eq!(
        hint(&changed("Cargo.lock"), true, Some(1), Some(0), None),
        Some(Hint::Lockfile)
    );
}

#[test]
fn hints_by_diff() {
    type Case<'a> = (&'a [(LineKind, &'a str)], Option<Hint>);
    let cases: &[Case] = &[
        (
            &[(LineKind::Added, "pub fn login(user: &str) {")],
            Some(Hint::Signature),
        ),
        (
            &[(LineKind::Removed, "fn check() -> bool {")],
            Some(Hint::Signature),
        ),
        (
            &[(LineKind::Added, "export const routes = [];")],
            Some(Hint::Signature),
        ),
        (
            &[(LineKind::Added, "    public func run() {")],
            Some(Hint::Signature),
        ),
        (
            &[(LineKind::Added, "def handle(request):")],
            Some(Hint::Signature),
        ),
        (
            &[(LineKind::Added, "pub(crate) struct Token;")],
            Some(Hint::Signature),
        ),
        // Only changed lines count.
        (
            &[
                (LineKind::Context, "pub fn login() {"),
                (LineKind::Added, "    x += 1;"),
            ],
            None,
        ),
        (&[(LineKind::Added, "    // fn in a comment")], None),
    ];
    for &(lines, want) in cases {
        assert_eq!(
            hint(
                &changed("src/a.rs"),
                false,
                Some(1),
                Some(1),
                Some(&text(lines))
            ),
            want,
            "{lines:?}"
        );
    }
}

#[test]
fn a_change_that_only_deletes_sinks() {
    let removed = text(&[(LineKind::Removed, "pub fn old() {}")]);
    assert_eq!(
        hint(
            &changed("src/a.rs"),
            false,
            Some(0),
            Some(3),
            Some(&removed)
        ),
        Some(Hint::OnlyDeletes)
    );
    // A binary file has no counts to tell by.
    assert_eq!(hint(&changed("logo.png"), false, None, None, None), None);
}

#[test]
fn a_mode_the_lines_dont_show_rises_whatever_the_path() {
    use FileMode::{Executable, Regular, Submodule, Symlink};
    let line = text(&[(LineKind::Added, "/etc/passwd")]);
    let cases = [
        (Some(Regular), Some(Executable), Some(Hint::Mode)),
        (Some(Executable), Some(Regular), Some(Hint::Mode)),
        (Some(Symlink), Some(Regular), Some(Hint::Mode)),
        (Some(Regular), Some(Symlink), Some(Hint::Symlink)),
        (None, Some(Symlink), Some(Hint::Symlink)),
        (Some(Symlink), Some(Symlink), Some(Hint::Symlink)),
        (None, Some(Submodule), Some(Hint::Submodule)),
        (Some(Submodule), Some(Submodule), Some(Hint::Submodule)),
        // Adding an executable, or deleting a symlink, changes no mode.
        (None, Some(Executable), Some(Hint::Docs)),
        (Some(Symlink), None, Some(Hint::Docs)),
        // Modes from before the hub kept them say nothing.
        (None, None, Some(Hint::Docs)),
    ];
    for (old, new, want) in cases {
        let file = moded("docs/README.md", old, new);
        assert_eq!(
            hint(&file, false, Some(1), Some(1), Some(&line)),
            want,
            "{old:?} -> {new:?}"
        );
    }
}

#[test]
fn sinks_weigh_less_than_rises() {
    let sinks = [
        Hint::Lockfile,
        Hint::Generated,
        Hint::Docs,
        Hint::Tests,
        Hint::OnlyDeletes,
    ];
    let rises = [
        Hint::Config,
        Hint::Signature,
        Hint::Migration,
        Hint::Mode,
        Hint::Symlink,
        Hint::Submodule,
    ];
    assert!(sinks.iter().all(|h| h.weight() < 0));
    assert!(rises.iter().all(|h| h.weight() > 0));
}
