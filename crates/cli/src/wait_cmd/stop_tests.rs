use super::*;

#[test]
fn names_the_cursor_and_the_command_that_resumes() {
    let stopped = Stopped {
        stopped: "SIGTERM",
        item: "app#3".into(),
        cursor: Some(12),
        resume: "mg wait app#3 --since 12".into(),
    };
    assert_eq!(
        stopped.text(),
        "mg wait: SIGTERM stopped the wait on app#3 before it had events; nothing after \
         cursor 12 was delivered, so resume with: mg wait app#3 --since 12"
    );
}

#[test]
fn without_a_cursor_leaves_it_to_the_hub() {
    let stopped = Stopped {
        stopped: "SIGINT",
        item: "app#3".into(),
        cursor: None,
        resume: "mg wait app#3".into(),
    };
    assert!(
        stopped
            .text()
            .ends_with("nothing was marked delivered, so resume with: mg wait app#3"),
        "{}",
        stopped.text()
    );
}
