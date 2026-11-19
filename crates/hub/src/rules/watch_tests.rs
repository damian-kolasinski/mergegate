use super::*;

fn name(project: &str) -> ProjectName {
    ProjectName::parse(project).unwrap()
}

fn text(text: &str) -> Read {
    Ok(Some(text.into()))
}

fn stale(seen: &mut Seen, read: Vec<(&str, Read)>) -> Vec<String> {
    let read = read.into_iter().map(|(p, text)| (name(p), text)).collect();
    seen.stale(read)
        .into_iter()
        .map(|(project, _)| project.to_string())
        .collect()
}

fn synced(seen: &mut Seen, synced: Vec<(&str, Read)>) {
    seen.synced(
        synced
            .into_iter()
            .map(|(p, text)| (name(p), text))
            .collect(),
    );
}

#[test]
fn syncs_a_file_only_once_its_text_changes() {
    let mut seen = Seen::default();
    assert_eq!(
        stale(&mut seen, vec![("app", text("a")), ("web", Ok(None))]),
        ["app", "web"],
        "never synced"
    );
    synced(&mut seen, vec![("app", text("a")), ("web", Ok(None))]);
    assert!(stale(&mut seen, vec![("app", text("a")), ("web", Ok(None))]).is_empty());

    assert_eq!(
        stale(&mut seen, vec![("app", text("b")), ("web", text("new"))]),
        ["app", "web"]
    );
    assert_eq!(
        stale(&mut seen, vec![("app", Ok(None))]),
        ["app"],
        "deleted"
    );
}

#[test]
fn retries_a_failed_sync_and_an_unreadable_file_every_poll() {
    let mut seen = Seen::default();
    stale(&mut seen, vec![("app", text("a"))]);
    // The sync failed, so nothing was recorded.
    assert_eq!(stale(&mut seen, vec![("app", text("a"))]), ["app"]);

    let unreadable = || Err(RulesFileError::Io("denied".into()));
    synced(&mut seen, vec![("app", text("a"))]);
    synced(&mut seen, vec![("app", unreadable())]);
    assert_eq!(stale(&mut seen, vec![("app", unreadable())]), ["app"]);
    // Readable again as the text synced before it wasn't, it syncs again.
    assert_eq!(stale(&mut seen, vec![("app", text("a"))]), ["app"]);
}

#[test]
fn syncs_every_file_each_full_round_and_forgets_removed_projects() {
    let mut seen = Seen::default();
    stale(&mut seen, vec![("app", text("a"))]);
    synced(&mut seen, vec![("app", text("a"))]);
    for _ in 2..FULL {
        assert!(stale(&mut seen, vec![("app", text("a"))]).is_empty());
    }
    assert_eq!(stale(&mut seen, vec![("app", text("a"))]), ["app"]);

    stale(&mut seen, Vec::new());
    assert!(seen.texts.is_empty());
}
