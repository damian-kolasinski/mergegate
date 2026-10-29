use super::*;

#[test]
fn names_the_skill_and_says_when_to_load_it() {
    let stub = text();
    assert!(is_ours(&stub));
    assert!(
        stub.contains(&format!("\ndescription: {}\n", guide::description())),
        "{stub}"
    );
    assert!(stub.contains("Run `mg skill show`"), "{stub}");
}

#[test]
fn the_whole_skill_is_ours_too() {
    assert!(is_ours(guide::FILES[0].1));
    assert!(!is_stub(guide::FILES[0].1));
    assert!(!is_ours("---\nname: other\n---\n"));
}

#[test]
fn a_skill_the_operator_wrote_under_the_name_is_not() {
    assert!(!is_ours(
        "---\nname: mergegate\ndescription: My own notes\n---\n"
    ));
}

#[test]
fn carries_the_wake_script_older_wakes_run() {
    assert!(wake_script().contains("\"wakes\", \"codex\""));
}
