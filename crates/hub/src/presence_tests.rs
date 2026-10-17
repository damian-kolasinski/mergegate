use super::*;

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

#[test]
fn an_agent_stays_there_through_the_grace_after_its_read() {
    let readers = Readers::default();
    let start = Instant::now();
    assert!(!readers.there(&id(), start));

    assert!(readers.begin(&id(), start));
    assert!(readers.there(&id(), start + GRACE * 10));

    let ended = start + GRACE;
    readers.end(&id(), ended);
    assert!(readers.there(&id(), ended + GRACE / 2));
    assert!(!readers.expire(&id(), ended + GRACE / 2));

    // The next wait of a loop comes back within the grace: no change.
    assert!(!readers.begin(&id(), ended + GRACE / 2));
    readers.end(&id(), ended + GRACE / 2);
    // The first read's timer finds the second's grace still running.
    assert!(!readers.expire(&id(), ended + GRACE));

    let last = ended + GRACE / 2;
    assert!(readers.expire(&id(), last + GRACE));
    assert!(!readers.there(&id(), last + GRACE));
    assert!(readers.begin(&id(), last + GRACE));
}

#[test]
fn an_agent_is_there_while_any_of_its_reads_is() {
    let readers = Readers::default();
    let start = Instant::now();
    assert!(readers.begin(&id(), start));
    assert!(!readers.begin(&id(), start));
    readers.end(&id(), start);
    assert!(!readers.expire(&id(), start + GRACE * 2));
    readers.end(&id(), start + GRACE * 2);
    assert!(readers.expire(&id(), start + GRACE * 3));
}
