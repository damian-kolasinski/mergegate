use super::*;
use crate::fake::FakeStore;

fn jev(model: &str, threshold: f64) -> JudgeSetting {
    JudgeSetting {
        provider: Provider::Jev {
            model: model.into(),
        },
        threshold,
    }
}

#[test]
fn parses_the_set_arguments() {
    let parse = JudgeSetting::parse;
    assert_eq!(parse("none", None, None, None).unwrap(), None);
    assert_eq!(
        parse("jev", None, None, None).unwrap(),
        Some(jev(DEFAULT_MODEL, DEFAULT_THRESHOLD))
    );
    assert_eq!(
        parse("jev", Some("jev-2"), None, Some(0.5)).unwrap(),
        Some(jev("jev-2", 0.5))
    );
    assert_eq!(
        parse("command", None, Some("claude -p"), Some(1.0)).unwrap(),
        Some(JudgeSetting {
            provider: Provider::Command {
                command: "claude -p".into(),
            },
            threshold: 1.0,
        })
    );
}

#[test]
fn rejects_options_that_dont_fit() {
    // provider, model, command, threshold, then the error.
    type Case<'a> = (
        &'a str,
        Option<&'a str>,
        Option<&'a str>,
        Option<f64>,
        &'a str,
    );
    let cases: &[Case] = &[
        ("gpt", None, None, None, "no judge \"gpt\""),
        ("none", Some("m"), None, None, "none takes no options"),
        ("none", None, None, Some(0.5), "none takes no options"),
        (
            "jev",
            None,
            Some("x"),
            None,
            "--command is only for the command",
        ),
        ("jev", Some(" "), None, None, "--model can't be empty"),
        ("command", None, None, None, "needs --command"),
        ("command", None, Some(""), None, "--command can't be empty"),
        (
            "command",
            Some("m"),
            Some("x"),
            None,
            "--model is only for the jev",
        ),
        ("jev", None, None, Some(0.0), "above 0 and at most 1"),
        ("jev", None, None, Some(1.5), "above 0 and at most 1"),
        ("jev", None, None, Some(f64::NAN), "above 0 and at most 1"),
    ];
    for &(provider, model, command, threshold, want) in cases {
        let err = JudgeSetting::parse(provider, model, command, threshold).unwrap_err();
        assert!(err.to_string().contains(want), "{provider}: {err}");
    }
}

#[test]
fn the_key_changes_with_what_judges_not_the_threshold() {
    assert_eq!(jev("jev-1", 0.8).key(), jev("jev-1", 0.3).key());
    assert_ne!(jev("jev-1", 0.8).key(), jev("jev-2", 0.8).key());
}

#[test]
fn sets_a_known_projects_judge() {
    let mut store = FakeStore::with_item(1);
    let app = ProjectName::parse("app").unwrap();
    set(&mut store, &app, Some(&jev("jev-1", 0.8))).unwrap();
    assert_eq!(store.judge_setting(&app).unwrap(), Some(jev("jev-1", 0.8)));
    set(&mut store, &app, None).unwrap();
    assert_eq!(store.judge_setting(&app).unwrap(), None);

    let web = ProjectName::parse("web").unwrap();
    let err = set(&mut store, &web, None).unwrap_err();
    assert_eq!(err.to_string(), "no project named web");
}
