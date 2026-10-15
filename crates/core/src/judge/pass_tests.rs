use super::*;
use crate::fake::{FakeStore, commit};
use crate::item::{FileMode, ItemState};
use crate::judge::Provider;
use crate::triage::{RulesStore, Tier, Triage};

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn app1() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn command(command: &str) -> JudgeSetting {
    JudgeSetting {
        provider: Provider::Command {
            command: command.into(),
        },
        threshold: 0.8,
        order: Default::default(),
    }
}

fn verdict(path: &str, importance: u8, promote: Option<f64>) -> Verdict {
    Verdict {
        path: path.into(),
        importance,
        reason: None,
        promote,
    }
}

#[test]
fn lists_newest_revisions_its_judge_hasnt_judged() {
    let mut store = FakeStore::with_item(2);
    assert_eq!(unjudged(&store, None).unwrap(), [], "no judge, nothing");

    let first = command("one");
    store.set_judge_setting(&app(), Some(&first)).unwrap();
    let want = Unjudged {
        item: app1(),
        revision: 2,
        setting: first.clone(),
    };
    assert_eq!(unjudged(&store, None).unwrap(), std::slice::from_ref(&want));
    assert_eq!(unjudged(&store, Some(&app())).unwrap(), [want]);
    let web = ProjectName::parse("web").unwrap();
    assert_eq!(unjudged(&store, Some(&web)).unwrap(), []);

    store
        .insert_verdicts(&app1(), 2, &first.key(), &[])
        .unwrap();
    assert_eq!(unjudged(&store, None).unwrap(), []);

    // Another judge's verdicts don't count.
    store
        .set_judge_setting(&app(), Some(&command("two")))
        .unwrap();
    assert_eq!(unjudged(&store, None).unwrap().len(), 1);

    store.set_state(&app1(), ItemState::Abandoned);
    assert_eq!(unjudged(&store, None).unwrap(), []);
}

#[test]
fn requests_the_whole_change() {
    let mut store = FakeStore::with_item(1);
    store
        .set_guide(&app1(), 1, Some("## Files\n- `src/login.rs`\n"))
        .unwrap();
    let trusted = Triage {
        tier: Tier::Trusted,
        by: None,
    };
    store
        .set_triage(&app1(), 1, &[("README.md".into(), trusted)])
        .unwrap();
    let asked = request(&store, &app1(), 1).unwrap().unwrap();
    assert_eq!(asked.title, "Add login");
    assert_eq!(asked.guide.as_deref(), Some("## Files\n- `src/login.rs`\n"));
    let readme = &asked.files[0];
    assert_eq!(
        (readme.path.as_str(), readme.change, readme.tier),
        ("README.md", Change::Modified, Tier::Trusted)
    );
    let login = &asked.files[1];
    assert_eq!(
        (login.path.as_str(), login.change, login.tier),
        ("src/login.rs", Change::Added, Tier::Review)
    );
    assert_eq!(
        [readme, login].map(|f| (f.old_mode, f.new_mode)),
        [
            (Some(FileMode::Regular), Some(FileMode::Regular)),
            (None, Some(FileMode::Regular)),
        ]
    );
    let blob = commit(2);
    assert_eq!(
        login.diff.as_deref(),
        Some(&*format!("@@ -0,0 +1,1 @@\n+{blob}\n"))
    );

    let gone = ItemId::parse("app#9").unwrap();
    assert_eq!(request(&store, &gone, 1).unwrap(), None);
    store.set_state(&app1(), ItemState::Merged);
    assert_eq!(request(&store, &app1(), 1).unwrap(), None);
}

#[test]
fn requests_only_an_open_items_newest_revision() {
    let store = FakeStore::with_item(2);
    assert!(request(&store, &app1(), 2).unwrap().is_some());
    assert_eq!(request(&store, &app1(), 1).unwrap(), None);
}

#[test]
fn accepts_one_verdict_per_known_file_in_range() {
    let store = FakeStore::with_item(1);
    let mut request = request(&store, &app1(), 1).unwrap().unwrap();
    request.files[0].tier = Tier::Trusted;
    let answer = vec![
        Verdict {
            reason: Some("\n  touches login  \nmore".into()),
            ..verdict("src/login.rs", 0, Some(0.9))
        },
        verdict("src/login.rs", 50, None),
        verdict("src/gone.rs", 50, None),
        verdict("README.md", 200, Some(1.5)),
    ];
    // login.rs's promotion is kept for when a rule trusts it.
    assert_eq!(
        accept(&request, answer),
        [
            verdict("README.md", 100, Some(1.0)),
            Verdict {
                reason: Some("touches login".into()),
                ..verdict("src/login.rs", 1, Some(0.9))
            },
        ]
    );
}
