use super::*;

#[test]
fn reads_what_the_program_prints() {
    let output = spawn(
        "sh",
        &["-c", "echo out; echo err >&2"],
        Duration::from_secs(30),
    )
    .unwrap();
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

#[test]
fn runs_a_program_that_hung_once_again() {
    let ran = std::env::temp_dir().join(format!("mg-tailscale-hung-{}", std::process::id()));
    let _ = std::fs::remove_file(&ran);
    // Hangs the first time, answers the second.
    let script = format!(
        "if [ -e '{0}' ]; then echo answered; else touch '{0}'; sleep 10; fi",
        ran.display()
    );
    // Seconds, for a machine busy running tests, as a shell can take that
    // long to start.
    let output = retried(|| spawn("sh", &["-c", &script], Duration::from_secs(3)));
    let _ = std::fs::remove_file(&ran);
    assert_eq!(output.unwrap().stdout, b"answered\n");
}

#[test]
fn gives_up_on_a_program_that_hangs_twice() {
    let mut attempts = 0;
    let err = retried(|| {
        attempts += 1;
        spawn("sleep", &["10"], Duration::from_millis(100))
    })
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::TimedOut);
    assert_eq!(attempts, 2);
}

#[test]
fn runs_a_program_that_failed_only_once() {
    let mut attempts = 0;
    let output = retried(|| {
        attempts += 1;
        spawn("sh", &["-c", "exit 3"], Duration::from_secs(30))
    })
    .unwrap();
    assert!(!output.status.success());
    assert_eq!(attempts, 1);
}
