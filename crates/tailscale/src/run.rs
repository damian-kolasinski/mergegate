//! Running the `tailscale` CLI.

use std::io::{ErrorKind, Read};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use mergegate_core::device::TailnetError;

/// Where the Mac app keeps its CLI when the operator never put it on `PATH`.
const MAC_APP: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";

/// The CLI answers in tens of milliseconds. One that hangs, as it can while
/// its daemon restarts, would otherwise hold a request, a change of listener
/// or the hub's start for good.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Its stdout, or why it failed in the CLI's own words.
pub(crate) fn tailscale(args: &[&str]) -> Result<Vec<u8>, TailnetError> {
    let output = match spawn("tailscale", args, TIMEOUT) {
        Err(e) if e.kind() == ErrorKind::NotFound => spawn(MAC_APP, args, TIMEOUT),
        output => output,
    }
    .map_err(|e| {
        let why = match e.kind() {
            ErrorKind::NotFound => "Tailscale isn't installed: no tailscale on PATH".to_owned(),
            _ => format!("can't run tailscale: {e}"),
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

/// Runs `program`, and kills it if it hasn't exited within `timeout`.
fn spawn(program: &str, args: &[&str], timeout: Duration) -> std::io::Result<Output> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Read while it runs: the status of a large tailnet outgrows the pipe,
    // and the CLI would block writing it.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let Some(status) = wait(&mut child, timeout)? else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(std::io::Error::new(
            ErrorKind::TimedOut,
            format!("no answer within {}s", timeout.as_secs_f32()),
        ));
    };
    Ok(Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
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
