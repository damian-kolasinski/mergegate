use super::*;

#[test]
fn reads_what_the_program_prints() {
    let output = spawn("sh", &["-c", "echo out; echo err >&2"], TIMEOUT).unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"out\n");
    assert_eq!(output.stderr, b"err\n");
}

#[test]
fn kills_a_program_that_hangs() {
    let started = Instant::now();
    let err = spawn("sleep", &["10"], Duration::from_millis(100)).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
}
