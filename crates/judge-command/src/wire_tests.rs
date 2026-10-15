use mergegate_core::item::ItemId;
use mergegate_core::judge::{Change, RequestFile};
use mergegate_core::triage::Tier;
use serde_json::json;

use super::*;

#[test]
fn writes_the_whole_change() {
    let request = Request {
        item: ItemId::parse("app#1").unwrap(),
        revision: 2,
        title: "Add login".into(),
        guide: None,
        files: vec![RequestFile {
            path: "src/login.rs".into(),
            old_path: Some("src/auth.rs".into()),
            change: Change::Renamed,
            additions: Some(3),
            deletions: Some(1),
            tier: Tier::Trusted,
            generated: false,
            diff: Some("@@ -1 +1 @@\n-a\n+b\n".into()),
        }],
    };
    let written = serde_json::to_value(input(&request)).unwrap();
    assert_eq!(
        written,
        json!({
            "item": "app#1",
            "revision": 2,
            "title": "Add login",
            "guide": null,
            "files": [{
                "path": "src/login.rs",
                "old_path": "src/auth.rs",
                "change": "renamed",
                "additions": 3,
                "deletions": 1,
                "tier": "trusted",
                "generated": false,
                "diff": "@@ -1 +1 @@\n-a\n+b\n",
            }],
        })
    );
}

#[test]
fn reads_verdicts_rounding_the_score_into_range() {
    let read = verdicts(
        br#"{"files": [
            {"path": "a.rs", "importance": 72.6, "reason": "auth"},
            {"path": "b.rs", "importance": 0, "promote": 0.9},
            {"path": "c.rs", "importance": 400, "extra": true}
        ]}"#,
    )
    .unwrap();
    let scores: Vec<(&str, u8)> = read.iter().map(|v| (&*v.path, v.importance)).collect();
    assert_eq!(scores, [("a.rs", 73), ("b.rs", 1), ("c.rs", 100)]);
    assert_eq!(read[0].reason.as_deref(), Some("auth"));
    assert_eq!(read[1].promote, Some(0.9));
}

#[test]
fn names_what_it_expected_when_the_output_is_wrong() {
    let err = verdicts(b"Sure! Here are the scores:").unwrap_err();
    assert!(
        err.0.contains(r#"{"files": [{"path", "importance"}]}"#),
        "{}",
        err.0
    );
}
