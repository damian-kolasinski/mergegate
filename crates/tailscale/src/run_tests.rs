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

#[test]
fn forces_cli_mode_for_subprocesses() {
    let output = spawn(
        "sh",
        &["-c", "test \"$TAILSCALE_BE_CLI\" = 1 && printf cli"],
        Duration::from_secs(30),
    )
    .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"cli");
}

#[test]
fn timeout_names_the_program_attempts_and_stderr() {
    let error = answer(
        &["whois", "--json", "100.64.0.1"],
        retried(|| {
            spawn(
                "sh",
                &["-c", "echo 'waiting for network extension' >&2; sleep 30"],
                Duration::from_secs(1),
            )
        }),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("tailscale whois"), "{error}");
    assert!(error.contains("after 2 attempts"), "{error}");
    assert!(error.contains("sh: no answer within 1s"), "{error}");
    assert!(error.contains("waiting for network extension"), "{error}");
}

#[test]
fn kills_launcher_children_and_closes_their_pipes() {
    for script in ["sleep 30 & wait", "sleep 30 & exit 0"] {
        // The child inherits both pipes. Joining the readers would block for
        // 30 seconds if cleanup killed only the launcher, or skipped success.
        let started = Instant::now();
        let result = spawn("sh", &["-c", script], Duration::from_millis(200));
        assert!(started.elapsed() < Duration::from_secs(10));
        if script.ends_with("wait") {
            assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
        } else {
            assert!(result.unwrap().status.success());
        }
    }
}

#[test]
fn cleanup_does_not_need_path_utilities() {
    const CHILD: &str = "MG_TAILSCALE_TEST_NO_PATH";
    if std::env::var_os(CHILD).is_some() {
        let started = Instant::now();
        let error = spawn(
            "/bin/sh",
            &["-c", "/bin/sleep 30 & wait"],
            Duration::from_millis(200),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(10));
        return;
    }
    // Isolate PATH from other tests without mutating process-wide env.
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "run::tests::cleanup_does_not_need_path_utilities",
        ])
        .env(CHILD, "1")
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
