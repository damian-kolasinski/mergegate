use super::*;

fn merged(commands: &[&str]) -> Event {
    Event {
        cursor: 9,
        item: "app#1".into(),
        name: "app#1".into(),
        revision: 2,
        source: Source::Operator,
        body: EventBody::ItemMerged {
            commit: "abc".into(),
            branch: "feature".into(),
            commands: commands.iter().map(|c| (*c).to_owned()).collect(),
        },
    }
}

#[test]
fn heads_an_event_with_its_type() {
    assert_eq!(
        event(&merged(&["git fetch mergegate"])),
        "[9] item.merged app#1 r2: the operator merged it as abc\n\n  \
         Clean up feature as agreed with the operator; by default:\n    \
         git fetch mergegate"
    );
}

#[test]
fn a_merge_from_a_hub_that_sends_no_cleanup_offers_none() {
    assert_eq!(
        event(&merged(&[])),
        "[9] item.merged app#1 r2: the operator merged it as abc"
    );
}

fn feedback(state: ItemState) -> Feedback {
    Feedback {
        item: "app#1".into(),
        name: "app#1".into(),
        state,
        events: vec![merged(&[])],
        cursor: 9,
    }
}

#[test]
fn the_cursor_line_names_the_command_that_reads_on() {
    let open = feedback(ItemState::Merging);
    let wait = describe(&open, &["wait"]);
    assert!(
        wait.ends_with("\n\ncursor 9 (mg wait app#1 --since 9 continues after it)"),
        "{wait}"
    );
    let operator = describe(&open, &["wait", "--for", "operator"]);
    assert!(
        operator
            .ends_with("\n\ncursor 9 (mg wait app#1 --for operator --since 9 continues after it)"),
        "{operator}"
    );
    let feedback = describe(&open, &["feedback"]);
    assert!(
        feedback.ends_with("\n\ncursor 9 (mg feedback app#1 --since 9 continues after it)"),
        "{feedback}"
    );
}

#[test]
fn a_closed_item_has_nothing_to_read_on_from() {
    for state in [ItemState::Merged, ItemState::Abandoned] {
        let text = describe(&feedback(state), &["wait"]);
        assert!(text.ends_with("\n\ncursor 9"), "{text}");
    }
}
