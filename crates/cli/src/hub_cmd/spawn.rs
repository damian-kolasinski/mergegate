//! Running `mg hub start --foreground` detached, in its own session, so the
//! hub outlives the command and the terminal that started it.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};

use hub::Home;

use super::io_error;
use crate::output::{CliError, exit};

pub(super) struct Spawned {
    child: Child,
    /// The child's stdout: its JSON outcome. Unnamed, so concurrent starts
    /// can't mix them up and an interrupted start leaves nothing behind.
    outcome: File,
}

impl Spawned {
    pub fn new(home: &Home, port: u16) -> Result<Self, CliError> {
        let log = File::options()
            .create(true)
            .append(true)
            .open(home.log())
            .map_err(|e| io_error("opening hub.log", e))?;
        let outcome = tempfile::tempfile_in(home.root())
            .map_err(|e| io_error("creating the start outcome file", e))?;
        let child_stdout = outcome
            .try_clone()
            .map_err(|e| io_error("creating the start outcome file", e))?;
        let exe = std::env::current_exe().map_err(|e| io_error("locating the mg binary", e))?;

        let mut cmd = Command::new(exe);
        cmd.args(["--json", "hub", "start", "--foreground", "--port"])
            .arg(port.to_string())
            .env(hub::HOME_ENV, home.root())
            .env_remove(crate::JSON_ENV)
            .current_dir(home.root())
            .stdin(Stdio::null())
            .stdout(child_stdout)
            .stderr(log);
        new_session(&mut cmd);
        close_inherited_on_exec();
        let child = cmd
            .spawn()
            .map_err(|e| io_error("starting the hub process", e))?;
        Ok(Self { child, outcome })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// The error the hub exited with, if it has exited.
    pub fn failure(&mut self, home: &Home) -> Option<CliError> {
        self.child.try_wait().ok().flatten()?;
        // The child moved the shared file offset.
        let mut printed = Vec::new();
        let _ = self.outcome.seek(SeekFrom::Start(0));
        let _ = self.outcome.read_to_end(&mut printed);
        Some(
            match serde_json::from_slice::<api::error::ErrorEnvelope>(&printed) {
                Ok(envelope) => CliError {
                    exit: exit::ERROR,
                    body: envelope.error,
                },
                Err(_) => CliError::new("hub_start_failed", "the hub exited while starting")
                    .hint(format!("see {}", home.log().display())),
            },
        )
    }

    pub fn abandon(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Marks every descriptor beyond stderr close-on-exec, so the hub holds
/// none that mg inherited. A caller's pipe left open in the hub keeps the
/// caller waiting for its end as long as the hub runs, and one leaks in
/// easily: macOS can't create a pipe close-on-exec, so a parent spawning on
/// several threads can hand one command's pipe to another.
#[allow(unsafe_code)]
fn close_inherited_on_exec() {
    let Ok(entries) = std::fs::read_dir("/dev/fd") else {
        return;
    };
    // Listed first: the listing's own descriptor is among them, closed by
    // the time they're marked.
    let fds: Vec<i32> = entries
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .filter(|&fd| fd > 2)
        .collect();
    for fd in fds {
        // SAFETY: fcntl only reads and sets a descriptor's flags; on one
        // closed since it was listed it fails with EBADF, which is ignored.
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags != -1 {
                libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
            }
        }
    }
}

#[allow(unsafe_code)]
fn new_session(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setsid is async-signal-safe and touches no parent memory, which
    // is all a pre_exec closure may do between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}
