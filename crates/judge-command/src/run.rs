//! Running the command: in its own process group, which is killed whole
//! when it's done, over its time or output, or the hub stops, so nothing it
//! started lingers.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use mergegate_core::agent;
use mergegate_core::judge::{JudgeError, Request};

/// More output than any verdicts need; past it the command is stopped.
const MAX_OUTPUT: u64 = 8 << 20;
/// What's kept of stderr, from its end, for the last line it said.
const STDERR_TAIL: usize = 64 << 10;

/// The command's stdout once it exits successfully. Setting `stop` stops it.
pub(crate) fn run(
    command: &str,
    dir: &Path,
    request: &Request,
    input: &[u8],
    timeout: Duration,
    stop: &AtomicBool,
) -> Result<Vec<u8>, JudgeError> {
    let failed = |e: &dyn std::fmt::Display| JudgeError(format!("the judge command: {e}"));
    let mut sh = Command::new("sh");
    for name in agent::SESSION_MARKERS {
        sh.env_remove(name);
    }
    let mut child = sh
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .env("MG_ITEM", request.item.to_string())
        .env("MG_REVISION", request.revision.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| failed(&e))?;
    // Writing and reading on their own threads, so a command that answers
    // before it has read everything can't deadlock on a full pipe.
    let mut stdin = child.stdin.take().expect("piped");
    let input = input.to_vec();
    let writer = thread::spawn(move || {
        // A command that doesn't read its input closes the pipe early.
        let _ = stdin.write_all(&input);
    });
    let over = Arc::new(AtomicBool::new(false));
    let stdout = read_stdout(child.stdout.take().expect("piped"), over.clone());
    let stderr = read_stderr(child.stderr.take().expect("piped"));

    let status = wait(&mut child, timeout, &over, stop);
    kill_group(&child);
    if !matches!(status, Ok(Some(_))) {
        // Reaps it; the kill makes this return at once.
        let _ = child.wait();
    }
    let _ = writer.join();
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    // Before the status: a command can write too much and exit before the
    // wait sees it, and its output is cut short either way.
    if over.load(Ordering::Relaxed) {
        return Err(JudgeError(format!(
            "the judge command wrote over {} MB and was stopped",
            MAX_OUTPUT >> 20
        )));
    }
    let status = match status {
        Ok(Some(status)) => status,
        Ok(None) if stop.load(Ordering::Relaxed) => {
            return Err(JudgeError(
                "the judge command was stopped with the hub".into(),
            ));
        }
        Ok(None) => {
            return Err(JudgeError(format!(
                "the judge command took over {}s and was stopped",
                timeout.as_secs_f64()
            )));
        }
        Err(e) => return Err(failed(&e)),
    };
    if !status.success() {
        let said = String::from_utf8_lossy(&stderr);
        let last = said.lines().rev().find(|l| !l.trim().is_empty());
        return Err(JudgeError(match last {
            Some(line) => format!("the judge command failed ({status}): {}", line.trim()),
            None => format!("the judge command failed ({status})"),
        }));
    }
    Ok(stdout)
}

/// Sets `over` and stops reading past [`MAX_OUTPUT`], so the wait can stop
/// the command rather than let it block on a full pipe until its time.
fn read_stdout(
    pipe: impl Read + Send + 'static,
    over: Arc<AtomicBool>,
) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut out = Vec::new();
        let _ = pipe.take(MAX_OUTPUT + 1).read_to_end(&mut out);
        if out.len() as u64 > MAX_OUTPUT {
            over.store(true, Ordering::Relaxed);
        }
        out
    })
}

/// Drains it all, so a chatty command never blocks on it, keeping the end.
fn read_stderr(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut out = Vec::new();
        let mut buf = [0; 8192];
        while let Ok(n @ 1..) = pipe.read(&mut buf) {
            out.extend_from_slice(&buf[..n]);
            if out.len() > 2 * STDERR_TAIL {
                out.drain(..out.len() - STDERR_TAIL);
            }
        }
        out
    })
}

/// None when it gave up first: over `timeout` or the output, or stopped.
fn wait(
    child: &mut Child,
    timeout: Duration,
    over: &AtomicBool,
    stop: &AtomicBool,
) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if over.load(Ordering::Relaxed)
            || stop.load(Ordering::Relaxed)
            || Instant::now() >= deadline
        {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// With `kill`, which takes a group as a negative ID, rather than a libc
/// call that would need `unsafe`. The group is gone already when nothing
/// the command started outlived it.
fn kill_group(child: &Child) {
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
