//! A wait in Claude Code ends with the session that started it. Without a
//! timeout, a wait whose session died without ending it would read the
//! item's next event for nobody, and the hub, seeing it handed to a reader,
//! would never run the item's wake for it. The session's process gone, the
//! wait hangs itself up as SIGHUP does: it says how to resume and exits 129,
//! having delivered nothing.

use std::time::Duration;

/// Set in the commands a Claude Code session runs: the session's process.
const CLAUDE_PID: &str = "CLAUDE_PID";

/// How often the session is looked for.
const EVERY: Duration = Duration::from_secs(5);

/// Watches the session named by `CLAUDE_PID`, if any. Call it after
/// [`super::stop::watch`], so the SIGHUP goes to its thread.
pub(crate) fn watch() {
    let Some(pid) = std::env::var(CLAUDE_PID)
        .ok()
        .and_then(|pid| pid.parse::<libc::pid_t>().ok())
        .filter(|&pid| pid > 0)
    else {
        return;
    };
    // Unwatched, the wait only ends as one without a session does.
    let _ = std::thread::Builder::new()
        .name("session".into())
        .spawn(move || {
            while alive(pid) {
                std::thread::sleep(EVERY);
            }
            hang_up();
        });
}

#[allow(unsafe_code)]
fn alive(pid: libc::pid_t) -> bool {
    // SAFETY: signal 0 sends nothing; it only checks that `pid` exists.
    let found = unsafe { libc::kill(pid, 0) } == 0;
    // A process of another user's exists all the same.
    found || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[allow(unsafe_code)]
fn hang_up() {
    // SAFETY: plain calls with this process's own ID and a valid signal.
    unsafe {
        libc::kill(libc::getpid(), libc::SIGHUP);
    }
}
