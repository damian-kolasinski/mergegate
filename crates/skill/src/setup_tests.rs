use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

use super::*;
use crate::status::status;

fn home() -> (tempfile::TempDir, Scope) {
    let dir = tempfile::tempdir().unwrap();
    let scope = Scope::user(dir.path().to_owned());
    (dir, scope)
}

fn changes(steps: &[Step]) -> Vec<(Kind, Vec<Agent>, Change)> {
    steps
        .iter()
        .map(|s| (s.kind, s.agents.clone(), s.change.clone()))
        .collect()
}

#[test]
fn installs_the_stub_once_per_folder_and_a_hook_per_agent() {
    let (dir, scope) = home();
    let steps = setup(&scope, &Agent::ALL, false, false);
    assert_eq!(
        changes(&steps),
        [
            (Kind::Skill, vec![Agent::Claude], Change::Installed),
            (
                Kind::Skill,
                vec![Agent::Codex, Agent::OpenCode],
                Change::Installed
            ),
            (Kind::Hook, vec![Agent::Claude], Change::Installed),
            (Kind::Hook, vec![Agent::Codex], Change::Installed),
            (Kind::Hook, vec![Agent::OpenCode], Change::Unsupported),
        ]
    );
    let stub = dir.path().join(".agents/skills/mergegate/SKILL.md");
    assert_eq!(fs::read_to_string(&stub).unwrap(), stub::text());
    let mode = fs::metadata(stub.parent().unwrap())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755);
    assert_eq!(
        steps[3].approval(),
        Some("trust the new hook with /hooks in Codex")
    );
    assert_eq!(steps[2].approval(), None);
    assert!(status(&scope).iter().all(|s| s.ready()));

    // Again, nothing changes; Codex keeps trusting its hook.
    let again = setup(&scope, &Agent::ALL, false, false);
    assert!(
        again
            .iter()
            .all(|s| matches!(s.change, Change::Unchanged | Change::Unsupported)),
        "{again:?}"
    );
}

#[test]
fn a_dry_run_changes_nothing() {
    let (dir, scope) = home();
    let steps = setup(&scope, &[Agent::Claude], false, true);
    assert_eq!(steps[0].change, Change::Installed);
    assert!(!dir.path().join(".claude").exists());
}

#[test]
fn replaces_an_older_link_and_drops_the_former_codex_copy() {
    let (dir, scope) = home();
    let data = dir.path().join(".mergegate/skill/mergegate");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("SKILL.md"), crate::guide::FILES[0].1).unwrap();
    fs::create_dir_all(dir.path().join(".claude/skills")).unwrap();
    fs::create_dir_all(dir.path().join(".codex/skills")).unwrap();
    let claude = dir.path().join(".claude/skills/mergegate");
    let former = dir.path().join(".codex/skills/mergegate");
    symlink(&data, &claude).unwrap();
    symlink(&data, &former).unwrap();

    let steps = setup(&scope, &[Agent::Claude, Agent::Codex], false, false);
    assert_eq!(steps[0].change, Change::Updated);
    assert_eq!(steps[1].change, Change::Installed);
    assert_eq!(steps[2].path.as_deref(), Some(former.as_path()));
    assert_eq!(steps[2].change, Change::Removed);
    assert!(!claude.is_symlink());
    // Codex no longer lists it, while a wake left with its script's path,
    // as an older guide had Codex agents leave, still runs.
    assert!(!former.is_symlink());
    assert!(!former.join("SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(former.join("scripts/codex_wake.py")).unwrap(),
        stub::wake_script()
    );
    assert!(claude.join("scripts/codex_wake.py").is_file());
    // The link's target is the data directory's, which stays.
    assert!(data.join("SKILL.md").is_file());

    // Removing takes the retired folder too.
    remove(&scope, &[Agent::Codex], false);
    assert!(!former.exists());
}

#[test]
fn a_former_folder_linked_to_another_agents_stays() {
    let (dir, scope) = home();
    fs::create_dir_all(dir.path().join(".claude/skills")).unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    symlink(
        dir.path().join(".claude/skills"),
        dir.path().join(".codex/skills"),
    )
    .unwrap();
    let steps = setup(&scope, &[Agent::Claude, Agent::Codex], false, false);
    assert!(
        steps.iter().all(|s| s.change != Change::Removed),
        "{steps:?}"
    );
    let claude = dir.path().join(".claude/skills/mergegate/SKILL.md");
    assert_eq!(fs::read_to_string(claude).unwrap(), stub::text());
}

#[test]
fn settings_linked_to_an_empty_dotfile_stay_linked() {
    let (dir, scope) = home();
    let real = dir.path().join("dotfiles/settings.json");
    fs::create_dir_all(real.parent().unwrap()).unwrap();
    fs::write(&real, "").unwrap();
    fs::create_dir_all(dir.path().join(".claude")).unwrap();
    let link = dir.path().join(".claude/settings.json");
    symlink(&real, &link).unwrap();
    setup(&scope, &[Agent::Claude], false, false);
    assert!(link.is_symlink());
    assert!(fs::read_to_string(&real).unwrap().contains("session-start"));

    // A link to a file not written yet stays a link too.
    let codex = dir.path().join(".codex/hooks.json");
    fs::create_dir_all(codex.parent().unwrap()).unwrap();
    let later = dir.path().join("dotfiles/hooks.json");
    symlink(&later, &codex).unwrap();
    setup(&scope, &[Agent::Codex], false, false);
    assert!(codex.is_symlink());
    assert!(later.is_file());
}

#[test]
fn a_current_skill_linked_to_its_former_place_stays() {
    let (dir, scope) = home();
    let former = dir.path().join(".codex/skills/mergegate");
    fs::create_dir_all(&former).unwrap();
    fs::write(former.join("SKILL.md"), stub::text()).unwrap();
    fs::create_dir_all(dir.path().join(".agents/skills")).unwrap();
    symlink(&former, dir.path().join(".agents/skills/mergegate")).unwrap();
    let steps = setup(&scope, &[Agent::Codex], false, false);
    assert!(
        steps.iter().all(|s| s.change != Change::Removed),
        "{steps:?}"
    );
    assert_eq!(
        fs::read_to_string(former.join("SKILL.md")).unwrap(),
        stub::text()
    );
    assert!(refresh(&scope).is_empty());
}

#[test]
fn leaves_a_skill_it_did_not_write_unless_forced() {
    let (dir, scope) = home();
    let theirs = dir.path().join(".claude/skills/mergegate");
    fs::create_dir_all(&theirs).unwrap();
    fs::write(theirs.join("SKILL.md"), "---\nname: theirs\n---\n").unwrap();
    let steps = setup(&scope, &[Agent::Claude], false, false);
    assert_eq!(steps[0].change, Change::Conflict);
    assert!(steps[0].change.failed());
    assert_eq!(steps[1].change, Change::Installed, "the hook goes ahead");
    assert!(
        fs::read_to_string(theirs.join("SKILL.md"))
            .unwrap()
            .contains("theirs")
    );

    let forced = setup(&scope, &[Agent::Claude], true, false);
    assert_eq!(forced[0].change, Change::Updated);
    assert_eq!(
        remove(&scope, &[Agent::Claude], false)[0].change,
        Change::Removed
    );
}

#[test]
fn keeps_the_settings_it_edits() {
    let (dir, scope) = home();
    let real = dir.path().join("dotfiles/settings.json");
    fs::create_dir_all(real.parent().unwrap()).unwrap();
    fs::write(&real, "{\n  \"model\": \"opus\"\n}\n").unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
    fs::create_dir_all(dir.path().join(".claude")).unwrap();
    let link = dir.path().join(".claude/settings.json");
    symlink(&real, &link).unwrap();

    setup(&scope, &[Agent::Claude], false, false);
    assert!(link.is_symlink(), "the dotfiles link stays");
    let text = fs::read_to_string(&real).unwrap();
    assert!(
        text.starts_with("{\n  \"model\": \"opus\",\n  \"hooks\""),
        "{text}"
    );
    let mode = fs::metadata(&real).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);

    remove(&scope, &[Agent::Claude], false);
    assert_eq!(
        fs::read_to_string(&real).unwrap(),
        "{\n  \"model\": \"opus\"\n}\n"
    );
}

#[test]
fn a_broken_settings_file_fails_only_its_hook() {
    let (dir, scope) = home();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    fs::write(dir.path().join(".codex/hooks.json"), "{ nope").unwrap();
    let steps = setup(&scope, &[Agent::Codex], false, false);
    assert_eq!(steps[0].change, Change::Installed);
    assert!(matches!(&steps[1].change, Change::Failed(why) if why.contains("JSON")));
    assert_eq!(
        fs::read_to_string(dir.path().join(".codex/hooks.json")).unwrap(),
        "{ nope"
    );
}

#[test]
fn refreshing_updates_only_what_an_mg_installed() {
    let (dir, scope) = home();
    assert!(refresh(&scope).is_empty(), "installs nothing new");

    setup(&scope, &[Agent::Claude], false, false);
    let stub = dir.path().join(".claude/skills/mergegate/SKILL.md");
    fs::write(
        &stub,
        "---\nname: mergegate\nmetadata:\n  mergegate-stub: \"0\"\n---\nolder\n",
    )
    .unwrap();
    let steps = refresh(&scope);
    assert_eq!(
        changes(&steps),
        [(Kind::Skill, vec![Agent::Claude], Change::Updated)]
    );
    assert_eq!(fs::read_to_string(&stub).unwrap(), stub::text());
}

#[test]
fn refreshing_keeps_the_copy_an_agent_not_set_up_still_reads() {
    let (dir, scope) = home();
    let former = dir.path().join(".codex/skills/mergegate");
    fs::create_dir_all(&former).unwrap();
    fs::write(former.join("SKILL.md"), crate::guide::FILES[0].1).unwrap();
    assert!(refresh(&scope).is_empty());
    assert!(former.join("SKILL.md").is_file());
}

#[test]
fn refreshing_leaves_a_copy_the_operator_may_have_edited() {
    let (dir, scope) = home();
    let copy = dir.path().join(".claude/skills/mergegate");
    fs::create_dir_all(&copy).unwrap();
    let edited = format!("{}\nMy own notes.\n", crate::guide::FILES[0].1);
    fs::write(copy.join("SKILL.md"), &edited).unwrap();
    assert!(refresh(&scope).is_empty());
    assert_eq!(fs::read_to_string(copy.join("SKILL.md")).unwrap(), edited);
    // Setup, which the operator runs, replaces it.
    assert_eq!(
        setup(&scope, &[Agent::Claude], false, false)[0].change,
        Change::Updated
    );
}

#[test]
fn a_former_folder_linked_to_the_current_one_is_no_former_copy() {
    let (dir, scope) = home();
    fs::create_dir_all(dir.path().join(".agents/skills")).unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    symlink(
        dir.path().join(".agents/skills"),
        dir.path().join(".codex/skills"),
    )
    .unwrap();
    let steps = setup(&scope, &[Agent::Codex], false, false);
    assert!(
        steps.iter().all(|s| s.change != Change::Removed),
        "{steps:?}"
    );
    assert!(
        dir.path()
            .join(".agents/skills/mergegate/SKILL.md")
            .is_file()
    );
    let codex = crate::status::status(&scope)
        .into_iter()
        .find(|s| s.agent == Agent::Codex)
        .unwrap();
    assert!(codex.ready(), "{codex:?}");
}

#[test]
fn a_conflict_keeps_the_former_copy_in_a_dry_run_too() {
    let (dir, scope) = home();
    let theirs = dir.path().join(".agents/skills/mergegate");
    fs::create_dir_all(&theirs).unwrap();
    fs::write(theirs.join("SKILL.md"), "---\nname: theirs\n---\n").unwrap();
    let former = dir.path().join(".codex/skills/mergegate");
    fs::create_dir_all(&former).unwrap();
    fs::write(former.join("SKILL.md"), crate::guide::FILES[0].1).unwrap();
    for dry_run in [true, false] {
        let steps = setup(&scope, &[Agent::Codex], false, dry_run);
        assert!(
            steps
                .iter()
                .all(|s| s.path.as_deref() != Some(former.as_path())),
            "{steps:?}"
        );
    }
    assert!(former.join("SKILL.md").is_file());
}

#[test]
fn removing_a_shared_folder_names_every_agent_reading_it() {
    let (_dir, scope) = home();
    setup(&scope, &[Agent::Codex], false, false);
    let steps = remove(&scope, &[Agent::Codex], false);
    assert_eq!(steps[0].agents, [Agent::Codex, Agent::OpenCode]);
}

#[test]
fn removing_leaves_what_it_did_not_install() {
    let (dir, scope) = home();
    let steps = remove(&scope, &Agent::ALL, false);
    assert!(
        steps.iter().all(|s| s.change == Change::Absent),
        "{steps:?}"
    );

    setup(&scope, &Agent::ALL, false, false);
    let steps = remove(&scope, &Agent::ALL, false);
    assert!(
        steps.iter().all(|s| s.change == Change::Removed),
        "{steps:?}"
    );
    assert!(!dir.path().join(".agents/skills/mergegate").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join(".codex/hooks.json")).unwrap(),
        "{}\n"
    );
}
