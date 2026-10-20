//! A wait stopped from outside. Agents' hosts end a command that runs past
//! their limit, or that they no longer track, with SIGTERM; killed silently,
//! a wait looks to its agent like one that is still quiet. Stopped this way,
//! `mg wait` prints how to resume and exits `128 + signal`, as a shell
//! reports a killed command. Nothing is lost: a wait marks events delivered
//! only once it printed them.

use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::output::{Outcome, emit};

/// What a stopped wait prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Stopped {
    /// The signal's name, as in `SIGTERM`.
    pub stopped: &'static str,
    pub item: String,
    /// The cursor the wait read after; none when it read after the
    /// delivered cursor, which the hub keeps.
    pub cursor: Option<u64>,
    /// The command that waits on from where this one stopped.
    pub resume: String,
}

impl Stopped {
    pub(crate) fn text(&self) -> String {
        let read = match self.cursor {
            Some(cursor) => format!("nothing after cursor {cursor} was delivered"),
            None => "nothing was marked delivered".to_owned(),
        };
        format!(
            "mg wait: {} stopped the wait on {} before it had events; {read}, so resume with: {}",
            self.stopped, self.item, self.resume
        )
    }
}

enum State {
    /// Waiting; a signal prints this.
    Waiting(Stopped),
    /// The wait has its outcome and prints it; a signal changes nothing.
    Done,
}

/// The wait's side of the signal thread.
pub(crate) struct Watch {
    state: Arc<Mutex<State>>,
}

/// SIGTERM, SIGINT and SIGHUP stop a wait.
const SIGNALS: [(libc::c_int, &str); 3] = [
    (libc::SIGTERM, "SIGTERM"),
    (libc::SIGINT, "SIGINT"),
    (libc::SIGHUP, "SIGHUP"),
];

/// Takes over the stopping signals for the rest of the process. Call it
/// before any other thread starts: the signals are blocked on this thread,
/// threads started later inherit that, and only the one started here takes
/// them. Where they can't be taken over, they kill the wait as before.
#[allow(unsafe_code)]
pub(crate) fn watch(json: bool, stopped: Stopped) -> Watch {
    let state = Arc::new(Mutex::new(State::Waiting(stopped)));
    // SAFETY: `set` is a plain C struct that `sigemptyset` initializes
    // before any other use; the signal numbers are valid.
    let set = unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for (signal, _) in SIGNALS {
            libc::sigaddset(&mut set, signal);
        }
        set
    };
    // SAFETY: `set` is initialized and outlives the call; no old mask is
    // asked for.
    let blocked = unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) };
    if blocked != 0 {
        return Watch { state };
    }
    let watched = Arc::clone(&state);
    let taken = std::thread::Builder::new()
        .name("signals".into())
        .spawn(move || {
            let mut signal: libc::c_int = 0;
            // SAFETY: `set` is initialized and `signal` is a valid place
            // for the result.
            if unsafe { libc::sigwait(&set, &mut signal) } != 0 {
                return;
            }
            let name = SIGNALS
                .iter()
                .find(|(s, _)| *s == signal)
                .map_or("a signal", |(_, name)| name);
            let state = watched.lock().unwrap_or_else(|e| e.into_inner());
            match &*state {
                // The outcome is on its way; the host gets it and exit 0.
                State::Done => return,
                State::Waiting(stopped) => {
                    let stopped = Stopped {
                        stopped: name,
                        ..stopped.clone()
                    };
                    let text = stopped.text();
                    let code = 128 + signal as u8;
                    // Printing to a closed terminal or pipe panics, as after
                    // SIGHUP; unwound, this thread would leave the signals
                    // blocked and the wait unkillable but by SIGKILL.
                    let _ = std::panic::catch_unwind(|| {
                        emit(json, Ok(Outcome::done(&stopped, text).exit(code)))
                    });
                }
            }
            std::process::exit(128 + signal);
        });
    if taken.is_err() {
        // SAFETY: as above; this undoes the block, so the signals kill.
        unsafe { libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut()) };
    }
    Watch { state }
}

impl Watch {
    /// What a signal from now on prints.
    pub(crate) fn waiting(&self, stopped: Stopped) {
        *self.lock() = State::Waiting(stopped);
    }

    /// The wait prints its own outcome from now on.
    pub(crate) fn done(&self) {
        *self.lock() = State::Done;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
