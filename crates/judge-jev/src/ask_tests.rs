use mergegate_core::item::{FileMode, ItemId};
use mergegate_core::judge::{Change, RequestFile};

use super::*;

fn request() -> Request {
    let file = |path: &str, tier| RequestFile {
        path: path.into(),
        old_path: None,
        change: Change::Modified,
        additions: Some(1),
        deletions: Some(1),
        tier,
        generated: false,
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        diff: Some("@@ -1,1 +1,1 @@\n-a\n+b\n".into()),
    };
    Request {
        item: ItemId::parse("app#1").unwrap(),
        revision: 1,
        title: "Work".into(),
        guide: None,
        files: vec![
            file("Cargo.lock", Tier::Trusted),
            file("src/a.rs", Tier::Review),
        ],
    }
}

#[test]
fn asks_for_a_score_and_a_look_at_trusted_files() {
    let questions = questions(&request(), &[0, 1]);
    let mut keys: Vec<&String> = questions.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["f1_importance", "f1_look", "f2_importance"]);
    let score = &questions["f2_importance"];
    assert_eq!(score["type"], "score");
    assert_eq!(score["criteria"].as_array().unwrap().len(), 10);
    assert!(score["instructions"].as_str().unwrap().contains("src/a.rs"));
    assert_eq!(questions["f1_look"]["type"], "noul");
}

#[test]
fn maps_scores_onto_1_to_100() {
    let answers = json!({
        "f1_importance": { "type": "score", "score": 0.0 },
        "f1_look": { "type": "noul", "noul": 0.92 },
        "f2_importance": { "type": "score", "score": 9.0 },
    });
    let read = verdicts(&request(), &[0, 1], &answers);
    let got: Vec<_> = read
        .iter()
        .map(|v| (v.path.as_str(), v.importance, v.promote))
        .collect();
    assert_eq!(
        got,
        [("Cargo.lock", 1, Some(0.92)), ("src/a.rs", 100, None)]
    );

    let middle = json!({ "f2_importance": { "score": 4.5 } });
    assert_eq!(verdicts(&request(), &[1], &middle)[0].importance, 51);
    let missing = json!({ "f2_importance": { "score": null } });
    assert_eq!(verdicts(&request(), &[1], &missing), []);
}
