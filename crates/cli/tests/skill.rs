//! Installing the skill: `mg skill install` writes the skill embedded in `mg` and
//! says where, for the agent to load from wherever its harness looks.

use testkit::Env;

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

#[test]
fn f3_install_writes_the_skill_under_the_home() {
    let env = env();
    let installed = env
        .mg(&["--json", "skill", "install"])
        .assert_code(0)
        .json();
    let path = env.home().join("skill/mergegate");
    assert_eq!(installed["path"], path.display().to_string());
    let files: Vec<_> = installed["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        [
            "SKILL.md",
            "references/setup.md",
            "references/authoring.md",
            "references/waiting.md",
            "references/operator.md"
        ]
    );
    for (file, contents) in skill::FILES {
        assert_eq!(&std::fs::read_to_string(path.join(file)).unwrap(), contents);
    }
    // The agent settles merged branches with its operator.
    assert!(
        installed["next"]
            .as_str()
            .unwrap()
            .contains("git branch -D")
    );
}

#[test]
fn f3_installing_again_replaces_the_folder() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path().to_str().unwrap();
    env.mg(&["skill", "install", "--dir", at]).assert_code(0);
    let stale = dir.path().join("mergegate/references/dropped.md");
    std::fs::write(&stale, "from an older mg").unwrap();

    let text = env.mg(&["skill", "install", "--dir", at]).assert_code(0);
    assert!(
        text.stdout.starts_with(&format!(
            "installed the mergegate skill in {at}/mergegate\n"
        )),
        "{text:?}"
    );
    assert!(!stale.exists());
    assert!(dir.path().join("mergegate/SKILL.md").is_file());
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(left.len(), 1, "no staging folder left behind");
}

#[test]
fn f3_install_leaves_a_folder_it_did_not_write() {
    let env = env();
    let dir = tempfile::tempdir().unwrap();
    let theirs = dir.path().join("mergegate");
    std::fs::create_dir(&theirs).unwrap();
    std::fs::write(theirs.join("notes.md"), "mine").unwrap();
    let refused = env
        .mg(&[
            "--json",
            "skill",
            "install",
            "--dir",
            dir.path().to_str().unwrap(),
        ])
        .assert_code(1)
        .json();
    assert_eq!(refused["error"]["code"], "not_a_skill");
    assert!(theirs.join("notes.md").is_file());
}
