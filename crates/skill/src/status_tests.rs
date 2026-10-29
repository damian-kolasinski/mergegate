use std::fs;
use std::os::unix::fs::symlink;

use super::*;

fn home() -> (tempfile::TempDir, Scope) {
    let dir = tempfile::tempdir().unwrap();
    let scope = Scope::user(dir.path().to_owned());
    (dir, scope)
}

fn of(scope: &Scope, agent: Agent) -> AgentStatus {
    status(scope)
        .into_iter()
        .find(|s| s.agent == agent)
        .unwrap()
}

#[test]
fn finds_agents_by_their_settings_folder() {
    let (dir, scope) = home();
    assert!(scope.found().is_empty());
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    fs::create_dir_all(dir.path().join(".config/opencode")).unwrap();
    assert_eq!(scope.found(), [Agent::Codex, Agent::OpenCode]);
}

#[test]
fn nothing_installed_is_missing() {
    let (dir, scope) = home();
    let claude = of(&scope, Agent::Claude);
    assert!(!claude.found);
    assert_eq!(claude.skill.state, State::Missing);
    assert_eq!(
        claude.skill.path,
        Some(dir.path().join(".claude/skills/mergegate"))
    );
    assert_eq!(claude.hook.state, State::Missing);
    let opencode = of(&scope, Agent::OpenCode);
    assert_eq!(opencode.hook.state, State::Unsupported);
    assert_eq!(opencode.hook.path, None);
}

#[test]
fn tells_ours_from_theirs() {
    let (dir, _) = home();
    let path = dir.path().join("mergegate");
    fs::create_dir(&path).unwrap();
    assert_eq!(skill_state(&path), State::Foreign);
    fs::write(
        path.join("SKILL.md"),
        "---\nname: mergegate\nmetadata:\n  mergegate-stub: \"0\"\n---\n",
    )
    .unwrap();
    assert_eq!(skill_state(&path), State::Outdated);
    fs::write(path.join("SKILL.md"), stub::text()).unwrap();
    assert_eq!(skill_state(&path), State::Current);
    fs::write(path.join("SKILL.md"), "---\nname: mine\n").unwrap();
    assert_eq!(skill_state(&path), State::Foreign);
    // The operator's own skill under the name is still theirs.
    let mine = "---\nname: mergegate\ndescription: My notes\n---\n";
    fs::write(path.join("SKILL.md"), mine).unwrap();
    assert_eq!(skill_state(&path), State::Foreign);
}

#[test]
fn a_link_an_older_mg_left_is_ours() {
    let (dir, _) = home();
    let target = dir.path().join("data/skill/mergegate");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("SKILL.md"), crate::guide::FILES[0].1).unwrap();
    let link = dir.path().join("mergegate");
    symlink(&target, &link).unwrap();
    assert_eq!(skill_state(&link), State::Outdated);
    // Its data directory gone, it holds nobody's skill.
    fs::remove_dir_all(dir.path().join("data")).unwrap();
    assert_eq!(skill_state(&link), State::Outdated);
}

#[test]
fn a_settings_file_that_isnt_json_is_unreadable() {
    let (dir, scope) = home();
    fs::create_dir_all(dir.path().join(".claude")).unwrap();
    fs::write(dir.path().join(".claude/settings.json"), "{ nope").unwrap();
    let State::Unreadable(why) = of(&scope, Agent::Claude).hook.state else {
        panic!("readable");
    };
    assert!(why.starts_with("isn't valid JSON"), "{why}");
}
