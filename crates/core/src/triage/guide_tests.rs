use super::*;
use crate::item::ChangedFile;
use crate::triage::{RuleId, Triage};

fn files(guide: &Guide) -> Vec<(&str, Option<&str>)> {
    guide
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.reason.as_deref()))
        .collect()
}

fn stat(path: &str, tier: Tier) -> FileStat {
    FileStat {
        file: ChangedFile {
            path: path.into(),
            old_path: None,
            old_blob: Some("old".into()),
            new_blob: Some("new".into()),
        },
        additions: Some(1),
        deletions: Some(0),
        generated: false,
        triage: Triage {
            tier,
            by: (tier == Tier::Trusted).then(|| TriagedBy::Rule {
                id: RuleId::new(2),
                why: "`*.lock` — lockfile".into(),
            }),
        },
    }
}

const GUIDE: &str = "\
Moves the token check into the middleware.

## Files

- `src/auth.rs` — the check now lives here
- `src/routes.rs`: drops the old check,
  which every route repeated
* `README.md`
- `docs/a.md` -- why

## Testing

- `src/not-a-file.rs` — outside the list
";

#[test]
fn reads_the_files_list() {
    let guide = Guide::parse(GUIDE);
    assert_eq!(
        files(&guide),
        [
            ("src/auth.rs", Some("the check now lives here")),
            (
                "src/routes.rs",
                Some("drops the old check, which every route repeated")
            ),
            ("README.md", None),
            ("docs/a.md", Some("why")),
        ]
    );
    assert_eq!(guide.position("README.md"), Some(2));
    assert_eq!(guide.position("src/not-a-file.rs"), None);
    assert_eq!(
        guide.reason("src/auth.rs"),
        Some("the check now lives here")
    );
}

#[test]
fn skips_code_and_other_headings() {
    let guide = Guide::parse(
        "# Files\n- `a.rs`\n## Files ##\n```\n## Other\n```\n- `b.rs` — kept\n### Files\n- `c.rs`\n",
    );
    assert_eq!(files(&guide), [("b.rs", Some("kept"))]);
}

#[test]
fn warns_about_what_it_cannot_place() {
    let text = "## Files\n- `src/a.rs`\n- `gone.rs`\n- `Cargo.lock` — bumped\n- `src/a.rs` again\n- no path here\n";
    let guide = Guide::parse(text);
    let changed = [
        stat("src/a.rs", Tier::Review),
        stat("Cargo.lock", Tier::Trusted),
    ];
    assert_eq!(
        guide.warnings(text, &changed),
        [
            "`src/a.rs` is listed twice; its first place counts",
            "\"- no path here\" names no `path`",
            "`gone.rs` isn't changed in this revision",
            "`Cargo.lock` is trusted by R2, and the guide only orders the files to review",
        ]
    );
}

#[test]
fn warns_when_there_is_no_list() {
    let text = "Just a summary.";
    assert_eq!(
        Guide::parse(text).warnings(text, &[]),
        ["the guide has no `## Files` list, so the files stay in path order"]
    );
    assert!(Guide::parse("").warnings("", &[]).is_empty());
}
