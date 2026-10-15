use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use mergegate_core::item::ItemId;

use super::*;

fn request() -> Request {
    Request {
        item: ItemId::parse("app#4").unwrap(),
        revision: 3,
        title: "Add login".into(),
        guide: None,
        files: Vec::new(),
    }
}

fn run_in(dir: &Path, command: &str, input: &[u8], timeout: Duration) -> Result<String, String> {
    run(
        command,
        dir,
        &request(),
        input,
        timeout,
        &AtomicBool::new(false),
    )
    .map(|out| String::from_utf8(out).unwrap())
    .map_err(|e| e.0)
}

#[test]
fn passes_the_input_and_the_item_and_runs_in_the_repo() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("marker"), "here").unwrap();
    let out = run_in(
        dir.path(),
        r#"printf '%s %s ' "$MG_ITEM" "$MG_REVISION"; cat marker; printf ' '; cat"#,
        b"{\"files\":[]}",
        Duration::from_secs(10),
    )
    .unwrap();
    assert_eq!(out, "app#4 3 here {\"files\":[]}");
}

#[test]
fn reads_a_large_input_while_the_command_writes() {
    // Larger than a pipe buffer both ways, so writing it all first would
    // deadlock.
    let dir = tempfile::tempdir().unwrap();
    let input = vec![b'x'; 1 << 20];
    let out = run_in(dir.path(), "cat", &input, Duration::from_secs(10)).unwrap();
    assert_eq!(out.len(), input.len());
}

#[test]
fn reports_a_failure_by_the_last_thing_it_said() {
    let dir = tempfile::tempdir().unwrap();
    let err = run_in(
        dir.path(),
        "echo starting >&2; echo 'no API key' >&2; exit 3",
        b"",
        Duration::from_secs(10),
    )
    .unwrap_err();
    assert!(err.contains("failed"), "{err}");
    assert!(err.ends_with(": no API key"), "{err}");
}

#[test]
fn stops_a_command_over_its_time_with_what_it_started() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    // The background sleep holds stdout open: unless the whole group is
    // killed, reading it would wait the full minute.
    let err = run_in(
        dir.path(),
        "sleep 60 & sleep 60",
        b"",
        Duration::from_millis(300),
    )
    .unwrap_err();
    assert!(err.contains("took over 0.3s"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn does_not_wait_on_what_a_finished_command_left_behind() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let out = run_in(
        dir.path(),
        "sleep 60 & echo done",
        b"",
        Duration::from_secs(30),
    )
    .unwrap();
    assert_eq!(out, "done\n");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn stops_a_command_that_writes_too_much() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let err = run_in(dir.path(), "yes", b"", Duration::from_secs(30)).unwrap_err();
    assert!(err.contains("wrote over 8 MB"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn stops_a_command_when_told_to() {
    let dir = tempfile::tempdir().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let stopping = stop.clone();
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        stopping.store(true, Ordering::Relaxed);
    });
    let err = run(
        "sleep 60 & sleep 60",
        dir.path(),
        &request(),
        b"",
        Duration::from_secs(60),
        &stop,
    )
    .unwrap_err();
    setter.join().unwrap();
    assert!(err.0.contains("stopped with the hub"), "{}", err.0);
    assert!(started.elapsed() < Duration::from_secs(10));
}
