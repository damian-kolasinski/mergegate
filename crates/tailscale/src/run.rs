//! Running the `tailscale` CLI.

use std::io::{ErrorKind, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use mergegate_core::device::TailnetError;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;

/// Where the Mac app keeps its CLI when the operator never put it on `PATH`.
const MAC_APP: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";

/// Its stdout, or why it failed in the CLI's own words. The CLI answers in
/// tens of milliseconds; one that hangs, as it can while its daemon restarts,
/// would otherwise hold a request, a change of listener or the hub's start
/// for good, so it is killed after `timeout`.
pub(crate) fn tailscale(args: &[&str], timeout: Duration) -> Result<Vec<u8>, TailnetError> {
    answer(args, run(args, timeout))
}

/// [`tailscale`], run once more if it timed out: now and then the Mac app's
/// CLI misses the timeout on a busy machine and answers at once when run
/// again. For a request, which can wait for that; not for the hub's start,
/// which `mg hub start` gives up on in 10 seconds.
pub(crate) fn tailscale_retried(args: &[&str], timeout: Duration) -> Result<Vec<u8>, TailnetError> {
    answer(args, retried(|| run(args, timeout)))
}

fn answer(args: &[&str], output: std::io::Result<Output>) -> Result<Vec<u8>, TailnetError> {
    let output = output.map_err(|e| {
        let why = match e.kind() {
            ErrorKind::NotFound => "Tailscale isn't installed: no tailscale on PATH".to_owned(),
            _ => format!("can't run tailscale {}: {e}", args[0]),
        };
        TailnetError::Unavailable(why)
    })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let why = match stderr.trim() {
        "" => output.status.to_string(),
        stderr => stderr.to_owned(),
    };
    Err(TailnetError::Unavailable(format!(
        "tailscale {} failed: {why}",
        args[0]
    )))
}

/// `attempt`, made once more if it timed out.
fn retried(mut attempt: impl FnMut() -> std::io::Result<Output>) -> std::io::Result<Output> {
    match attempt() {
        Err(first) if first.kind() == ErrorKind::TimedOut => attempt().map_err(|last| {
            std::io::Error::new(last.kind(), format!("after 2 attempts: {first}; {last}"))
        }),
        output => output,
    }
}

/// The CLI on `PATH`, or else the Mac app's.
fn run(args: &[&str], timeout: Duration) -> std::io::Result<Output> {
    match spawn("tailscale", args, timeout) {
        Err(e) if e.kind() == ErrorKind::NotFound => spawn(MAC_APP, args, timeout),
        output => output,
    }
}

/// Runs `program`, and kills it if it hasn't exited within `timeout`.
fn spawn(program: &str, args: &[&str], timeout: Duration) -> std::io::Result<Output> {
    let mut child = Command::new(program)
        .args(args)
        // The macOS app otherwise guesses GUI vs CLI from shell variables
        // that a launchd service need not have.
        .env("TAILSCALE_BE_CLI", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    // Read while it runs: the status of a large tailnet outgrows the pipe,
    // and the CLI would block writing it.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = wait(&mut child, timeout);
    // PATH may name a shell launcher. Killing just that shell leaves its
    // Tailscale child alive, holding the pipes and overlapping the retry.
    kill_group(&mut child);
    if !matches!(status, Ok(Some(_))) {
        let _ = child.wait();
    }
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    let Some(status) = status? else {
        let tail = &stderr[stderr.len().saturating_sub(2048)..];
        let detail = String::from_utf8_lossy(tail);
        let detail = detail.trim();
        let detail = if detail.is_empty() {
            String::new()
        } else {
            format!("; stderr: {detail}")
        };
        return Err(std::io::Error::new(
            ErrorKind::TimedOut,
            format!(
                "{program}: no answer within {}s{detail}",
                timeout.as_secs_f32()
            ),
        ));
    };
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn kill_group(child: &mut Child) {
    // Unix process IDs fit pid_t. The child's own group was set at spawn;
    // signal it directly so cleanup also works with a minimal service PATH.
    if killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL).is_err() {
        let _ = child.kill();
    }
}

fn wait(child: &mut Child, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
