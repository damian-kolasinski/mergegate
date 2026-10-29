use super::*;

const THEIRS: &str = r#"{
  "model": "opus",
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup",
        "hooks": [
          { "type": "command", "command": "other-tool start", "timeout": 5 }
        ]
      }
    ],
    "Stop": []
  }
}"#;

#[test]
fn the_command_never_changes() {
    // Codex trusts a hook by its hash: changing this stops it running.
    assert_eq!(
        command(Agent::Codex),
        "/bin/sh -c 'mergegate skill session-start --agent codex 2>/dev/null || true'"
    );
}

#[test]
fn installs_after_the_operators_hooks() {
    let mut settings = Json::parse(THEIRS).unwrap();
    assert_eq!(state(&settings, Agent::Claude), HookState::Missing);
    install(&mut settings, Agent::Claude).unwrap();
    assert_eq!(state(&settings, Agent::Claude), HookState::Current);
    let printed = settings.print();
    assert!(
        printed.starts_with("{\n  \"model\": \"opus\",\n  \"hooks\""),
        "{printed}"
    );
    let other = printed.find("other-tool start").unwrap();
    let ours = printed.find(MARKER).unwrap();
    assert!(other < ours, "{printed}");
}

#[test]
fn installs_into_empty_settings() {
    let mut settings = Json::parse("{}").unwrap();
    install(&mut settings, Agent::Codex).unwrap();
    assert_eq!(state(&settings, Agent::Codex), HookState::Current);
    // The hook names its agent.
    assert_eq!(state(&settings, Agent::Claude), HookState::Outdated);
}

#[test]
fn installing_again_replaces_an_older_hook() {
    let older = r#"{"hooks": {"SessionStart": [
        {"hooks": [{"type": "command", "command": "mergegate skill session-start --agent claude"}]},
        {"hooks": [{"type": "command", "command": "other-tool start"},
                   {"type": "command", "command": "mergegate skill session-start"}]}
    ]}}"#;
    let mut settings = Json::parse(older).unwrap();
    assert_eq!(state(&settings, Agent::Claude), HookState::Outdated);
    install(&mut settings, Agent::Claude).unwrap();
    assert_eq!(state(&settings, Agent::Claude), HookState::Current);
    let groups = session_start(&settings);
    assert_eq!(groups.len(), 2, "{}", settings.print());
    assert_eq!(
        handlers(&groups[0]).len(),
        1,
        "the other tool's handler stays"
    );
}

#[test]
fn removing_leaves_the_operators_hooks() {
    let mut settings = Json::parse(THEIRS).unwrap();
    let before = settings.clone();
    assert!(!remove(&mut settings).unwrap());
    install(&mut settings, Agent::Claude).unwrap();
    assert!(remove(&mut settings).unwrap());
    assert_eq!(settings, before);
}

#[test]
fn removing_the_only_hook_removes_what_held_it() {
    let mut settings = Json::parse(r#"{"model": "opus"}"#).unwrap();
    install(&mut settings, Agent::Claude).unwrap();
    remove(&mut settings).unwrap();
    assert_eq!(settings.print(), "{\n  \"model\": \"opus\"\n}\n");
}

#[test]
fn leaves_settings_of_another_shape() {
    let mut settings = Json::parse(r#"{"hooks": {"SessionStart": "nope"}}"#).unwrap();
    assert!(install(&mut settings, Agent::Claude).is_err());
    let mut settings = Json::parse("[]").unwrap();
    assert!(install(&mut settings, Agent::Claude).is_err());
}
