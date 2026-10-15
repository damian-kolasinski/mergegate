use super::*;

fn merged(commands: &[&str]) -> Event {
    Event {
        cursor: 9,
        item: "app#1".into(),
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
