//! `mg hub start | stop | status`.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use hub::{Home, HubError, HubInfo, StoreError};
use serde::Serialize;

use crate::output::{CliError, Outcome, exit};

const START_TIMEOUT: Duration = Duration::from_secs(10);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(10);

#[derive(Serialize, Default)]
struct HubStatus {
    running: bool,
    home: String,
    /// Set by `start`: whether this call started the hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    started: Option<bool>,
    /// Set by `stop`: whether this call stopped the hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    stopped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<u32>,
    /// Whether this `mg` speaks the running hub's API version.
    #[serde(skip_serializing_if = "Option::is_none")]
    compatible: Option<bool>,
    /// A hub process holds the data directory but doesn't answer.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    unresponsive: bool,
}

struct Running {
    info: HubInfo,
    health: api::Health,
}

impl Running {
    fn url(&self) -> String {
        format!("http://{}", self.info.addr)
    }

    fn compatible(&self) -> bool {
        self.health.api_version == api::API_VERSION
    }

    fn status(&self, home: &Home) -> HubStatus {
        HubStatus {
            running: true,
            home: home_str(home),
            url: Some(self.url()),
            pid: Some(self.health.pid),
            version: Some(self.health.version.clone()),
            api_version: Some(self.health.api_version),
            schema_version: Some(self.health.schema_version),
            compatible: Some(self.compatible()),
            ..HubStatus::default()
        }
    }

    fn describe(&self) -> String {
        let h = &self.health;
        let mut text = format!(
            "{} (pid {}, mg {}, schema {})",
            self.url(),
            h.pid,
            h.version,
            h.schema_version
        );
        if !self.compatible() {
            text.push_str(&format!(
                "\nwarning: the hub speaks API v{}, this mg speaks v{}; \
                 restart it with this mg: mg hub stop && mg hub start",
                h.api_version,
                api::API_VERSION
            ));
        }
        text
    }
}

pub fn start(foreground: bool, port: u16) -> Result<Outcome, CliError> {
    let home = home()?;
    if foreground {
        return start_foreground(home, port);
    }
    home.ensure()
        .map_err(|e| io_error("creating the data directory", e))?;
    if let Some(running) = probe(&home) {
        if !running.compatible() {
            return Err(incompatible(&running));
        }
        return Ok(start_outcome(&home, &running, false));
    }

    let mut spawned: Option<Spawned> = None;
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        // While the home is locked another hub is starting, stopping or hung:
        // wait for it instead of racing it, and start ours once it is free.
        if spawned.is_none() && !home.is_locked() {
            spawned = Some(Spawned::new(&home, port)?);
        }
        if let Some(running) = probe(&home) {
            let ours = spawned
                .as_ref()
                .is_some_and(|s| s.child.id() == running.health.pid);
            if !ours && let Some(s) = spawned.as_mut() {
                // Another start won while ours waits for the lock. Ours must
                // not take it later, after the winner stops, as a hub no one
                // asked for.
                s.abandon();
            }
            return Ok(start_outcome(&home, &running, ours));
        }
        if let Some(s) = spawned.as_mut()
            && let Ok(Some(_)) = s.child.try_wait()
        {
            let err = s.failure(&home);
            if err.body.code != "hub_locked" {
                return Err(err);
            }
            // A concurrent `mg hub start` won the race; wait for its hub.
            spawned = None;
        }
        if Instant::now() > deadline {
            if spawned.is_none() && home.is_locked() {
                return Err(unresponsive(&home));
            }
            return Err(CliError::new(
                "hub_start_timeout",
                format!(
                    "the hub did not answer within {START_TIMEOUT:?}; it may still be starting"
                ),
            )
            .hint(format!("mg hub status; see {}", home.log().display()))
            .exit(exit::TIMEOUT));
        }
        std::thread::sleep(POLL);
    }
}

fn start_outcome(home: &Home, running: &Running, started: bool) -> Outcome {
    let status = HubStatus {
        started: Some(started),
        ..running.status(home)
    };
    let verb = if started {
        "started"
    } else {
        "already running"
    };
    Outcome::done(&status, format!("hub {verb} at {}", running.describe()))
}

fn start_foreground(home: Home, port: u16) -> Result<Outcome, CliError> {
    hub::init_logging();
    let root = home_str(&home);
    hub::run(home, port).map_err(hub_error)?;
    let status = HubStatus {
        home: root,
        stopped: Some(true),
        ..HubStatus::default()
    };
    Ok(Outcome::done(&status, "hub stopped"))
}

/// A `mg hub start --foreground` started in its own session, so it outlives
/// this command and the terminal. Its logs (stderr) are appended to the home's
/// `hub.log`; its JSON outcome (stdout) goes to an unnamed file of this
/// command's own, so a failure is never confused with another start's and
/// nothing is left behind if this command is interrupted.
struct Spawned {
    child: Child,
    outcome: File,
}

impl Spawned {
    fn new(home: &Home, port: u16) -> Result<Self, CliError> {
        let log = File::options()
            .create(true)
            .append(true)
            .open(home.log())
            .map_err(|e| io_error("opening hub.log", e))?;
        let outcome = tempfile::tempfile_in(home.root())
            .map_err(|e| io_error("creating the start outcome file", e))?;
        let outcome_file = outcome
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
            .stdout(outcome_file)
            .stderr(log);
        new_session(&mut cmd);
        let child = cmd
            .spawn()
            .map_err(|e| io_error("starting the hub process", e))?;
        Ok(Self { child, outcome })
    }

    /// The error the hub printed before exiting, relayed as-is.
    fn failure(&mut self, home: &Home) -> CliError {
        // The child shared this file's offset; it has exited, so rewind.
        let mut printed = Vec::new();
        let _ = self.outcome.seek(SeekFrom::Start(0));
        let _ = self.outcome.read_to_end(&mut printed);
        match serde_json::from_slice::<api::ErrorEnvelope>(&printed) {
            Ok(envelope) => CliError {
                exit: exit::ERROR,
                body: envelope.error,
            },
            Err(_) => CliError::new("hub_start_failed", "the hub exited while starting")
                .hint(format!("see {}", home.log().display())),
        }
    }

    fn abandon(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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

pub fn stop() -> Result<Outcome, CliError> {
    let home = home()?;
    // A hub that holds the lock without answering is starting or stopping;
    // give it until the deadline to do either.
    let wait_until = Instant::now() + STOP_TIMEOUT;
    let running = loop {
        if let Some(running) = probe(&home) {
            break running;
        }
        if !home.is_locked() {
            let status = HubStatus {
                home: home_str(&home),
                stopped: Some(false),
                ..HubStatus::default()
            };
            return Ok(Outcome::done(&status, "hub not running"));
        }
        if Instant::now() > wait_until {
            return Err(unresponsive(&home));
        }
        std::thread::sleep(POLL);
    };

    agent()
        .post(format!("{}/api/hub/shutdown", running.url()))
        .header(api::CLIENT_HEADER, env!("CARGO_PKG_VERSION"))
        .send_empty()
        .map_err(|e| {
            CliError::new(
                "hub_unreachable",
                format!("asking the hub to stop failed: {e}"),
            )
            .hint(format!("kill {}", running.health.pid))
            .exit(exit::UNAVAILABLE)
        })?;
    let deadline = Instant::now() + STOP_TIMEOUT;
    while home.is_locked() {
        if Instant::now() > deadline {
            return Err(CliError::new(
                "hub_stop_timeout",
                format!("the hub did not stop within {STOP_TIMEOUT:?}"),
            )
            .hint(format!("kill {}", running.health.pid))
            .exit(exit::TIMEOUT));
        }
        std::thread::sleep(POLL);
    }
    let status = HubStatus {
        home: home_str(&home),
        stopped: Some(true),
        ..HubStatus::default()
    };
    Ok(Outcome::done(&status, "hub stopped"))
}

pub fn status() -> Result<Outcome, CliError> {
    let home = home()?;
    if let Some(running) = probe(&home) {
        return Ok(Outcome::done(
            &running.status(&home),
            format!("hub running at {}", running.describe()),
        ));
    }
    let unresponsive = home.is_locked();
    let status = HubStatus {
        home: home_str(&home),
        unresponsive,
        pid: home
            .read_info()
            .map(|info| info.pid)
            .filter(|_| unresponsive),
        ..HubStatus::default()
    };
    let text = if unresponsive {
        "hub not responding (a hub process holds the data directory)"
    } else {
        "hub not running"
    };
    Ok(Outcome::done(&status, text))
}

/// The hub this home points to, if it answers its health check and is the
/// process `hub.json` names. (Its home path is not compared: the same
/// directory can be spelled many ways through symlinks.)
fn probe(home: &Home) -> Option<Running> {
    let info = home.read_info()?;
    let health: api::Health = agent()
        .get(format!("http://{}/api/health", info.addr))
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    (health.pid == info.pid).then_some(Running { info, health })
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .proxy(None)
        .build()
        .into()
}

fn home() -> Result<Home, CliError> {
    Home::from_env().map_err(|e| {
        CliError::new("invalid_home", e.to_string())
            .hint(format!("set {} to an absolute path", hub::HOME_ENV))
    })
}

fn home_str(home: &Home) -> String {
    home.root().display().to_string()
}

fn io_error(context: &str, e: io::Error) -> CliError {
    CliError::new("io_error", format!("{context}: {e}"))
}

fn incompatible(running: &Running) -> CliError {
    CliError::new(
        "hub_incompatible",
        format!(
            "the running hub speaks API v{}, this mg speaks v{}",
            running.health.api_version,
            api::API_VERSION
        ),
    )
    .hint("mg hub stop && mg hub start")
}

fn unresponsive(home: &Home) -> CliError {
    let pid = home.read_info().map(|info| info.pid);
    let hint = match pid {
        Some(pid) => format!("kill {pid}, then mg hub start"),
        None => format!(
            "stop the process holding {}/hub.lock",
            home.root().display()
        ),
    };
    CliError::new(
        "hub_unresponsive",
        "a hub process holds the data directory but doesn't answer",
    )
    .hint(hint)
    .exit(exit::UNAVAILABLE)
}

fn hub_error(e: HubError) -> CliError {
    let message = e.to_string();
    match e {
        HubError::Locked(_) => CliError::new("hub_locked", message).hint("mg hub status"),
        HubError::PortInUse(_) => {
            CliError::new("port_in_use", message).hint("mg hub start --port N")
        }
        HubError::Store(StoreError::SchemaTooNew { .. }) => {
            CliError::new("schema_too_new", message).hint(
                "install the mg version that last ran this hub, or restore a hub.db.bak-* backup",
            )
        }
        HubError::Store(_) => CliError::new("store_error", message),
        HubError::Io { .. } => CliError::new("io_error", message),
    }
}
