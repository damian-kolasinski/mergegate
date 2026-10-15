//! The default home's hub as a user service: launchd on macOS, systemd on
//! Linux. It starts at login and comes back after a crash. A hub with
//! `MERGEGATE_HOME` set (dev builds, tests) never gets one, so it can't take
//! the place of the installed hub's.

mod launchd;
mod systemd;

use std::fs;
use std::path::{Path, PathBuf};

use hub::Home;
use serde::Serialize;

use super::io_error;
use super::report::exited;
use crate::output::CliError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Manager {
    Launchd,
    Systemd,
}

impl Manager {
    /// The service manager on this OS, whether or not it can run the hub.
    fn here() -> Option<Manager> {
        if cfg!(target_os = "macos") {
            Some(Manager::Launchd)
        } else if cfg!(target_os = "linux") {
            Some(Manager::Systemd)
        } else {
            None
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Manager::Launchd => "launchd",
            Manager::Systemd => "systemd",
        }
    }

    fn path(self, user_home: &Path) -> PathBuf {
        match self {
            Manager::Launchd => launchd::path(user_home),
            Manager::Systemd => systemd::path(user_home),
        }
    }

    fn render(self, spec: &Spec) -> String {
        match self {
            Manager::Launchd => launchd::render(spec),
            Manager::Systemd => systemd::render(spec),
        }
    }

    fn exe(self, unit: &str) -> Option<PathBuf> {
        match self {
            Manager::Launchd => launchd::exe(unit),
            Manager::Systemd => systemd::exe(unit),
        }
    }

    fn load(self, unit: &Path, changed: bool) -> Result<(), String> {
        match self {
            Manager::Launchd => launchd::load(unit, changed),
            Manager::Systemd => systemd::load(unit, changed),
        }
    }

    fn unload(self) -> Result<(), String> {
        match self {
            Manager::Launchd => launchd::unload(),
            Manager::Systemd => systemd::unload(),
        }
    }

    /// Stops the hub without uninstalling: the next login starts it again.
    fn halt(self) {
        match self {
            // A plist in LaunchAgents is loaded again at login.
            Manager::Launchd => {
                let _ = launchd::unload();
            }
            Manager::Systemd => systemd::stop(),
        }
    }
}

/// What a unit runs: `mg hub start --foreground` for the default home.
pub(super) struct Spec {
    pub exe: PathBuf,
    pub port: u16,
    /// The working directory.
    pub home: PathBuf,
    /// The hub's `--json` outcome, read back when a start fails.
    pub outcome: PathBuf,
    pub log: PathBuf,
    /// The starting shell's: service managers start with a minimal one, and
    /// the hub runs gh, a recent git and wake commands from it.
    pub path_env: String,
}

impl Spec {
    pub fn args(&self) -> Vec<String> {
        let start = ["--json", "hub", "start", "--foreground", "--port"];
        std::iter::once(self.exe.display().to_string())
            .chain(start.iter().map(|arg| (*arg).to_owned()))
            .chain(std::iter::once(self.port.to_string()))
            .collect()
    }
}

/// An installed unit, as `mg hub status` and `mg doctor` report it.
#[derive(Debug, Serialize)]
pub(crate) struct Installed {
    pub manager: Manager,
    pub unit: String,
    /// The binary it runs, absent when the file isn't one mg wrote.
    pub exe: Option<String>,
}

/// Whether the hub at the home `mg` uses runs as a service.
pub(crate) fn applies() -> bool {
    std::env::var_os(hub::HOME_ENV).is_none_or(|v| v.is_empty())
}

/// Whether this machine has a service manager `mg hub start` can use.
pub(crate) fn usable() -> bool {
    match Manager::here() {
        Some(Manager::Launchd) => true,
        Some(Manager::Systemd) => systemd::usable(),
        None => false,
    }
}

/// The unit installed for this user, if any.
pub(crate) fn installed() -> Option<Installed> {
    let manager = Manager::here()?;
    let path = manager.path(&user_home()?);
    let unit = fs::read_to_string(&path).ok()?;
    Some(Installed {
        manager,
        unit: path.display().to_string(),
        exe: manager.exe(&unit).map(|exe| exe.display().to_string()),
    })
}

/// A service told to start the hub.
pub(super) struct Loaded {
    manager: Manager,
    outcome: PathBuf,
}

impl Loaded {
    /// The error the hub exited with, once it printed one.
    pub fn failure(&self, home: &Home) -> Option<CliError> {
        let printed = fs::read(&self.outcome).ok().filter(|p| !p.is_empty())?;
        Some(exited(&printed, home))
    }

    /// Stops the manager retrying a hub that fails to start. The unit stays,
    /// so the next login tries again.
    pub fn stop_retrying(&self) {
        self.manager.halt();
    }
}

/// Writes the unit for `mg hub start --port <port>` and has the manager
/// start it, or says why it can't, leaving no unit behind.
pub(super) fn load(home: &Home, port: u16) -> Result<Loaded, String> {
    let manager = Manager::here().ok_or("this OS has no service manager mg supports")?;
    if manager == Manager::Systemd && !systemd::usable() {
        return Err("systemd has no user session here".into());
    }
    let user_home = user_home().ok_or("HOME is not set")?;
    let exe = std::env::current_exe().map_err(|e| format!("locating the mg binary: {e}"))?;
    let spec = Spec {
        exe,
        port,
        home: home.root().to_owned(),
        outcome: outcome(home),
        log: home.log(),
        path_env: std::env::var("PATH").unwrap_or_default(),
    };
    let path = manager.path(&user_home);
    let unit = manager.render(&spec);
    let changed = fs::read_to_string(&path).ok().as_deref() != Some(unit.as_str());
    let write = || -> std::io::Result<()> {
        if changed {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&path, &unit)?;
        }
        // The manager opens it anew for each start, so whatever is in it
        // after this is this start's.
        fs::write(&spec.outcome, "")
    };
    write().map_err(|e| format!("writing {}: {e}", path.display()))?;
    if let Err(e) = manager.load(&path, changed) {
        let _ = manager.unload();
        let _ = fs::remove_file(&path);
        if manager == Manager::Systemd {
            systemd::forget();
        }
        return Err(e);
    }
    Ok(Loaded {
        manager,
        outcome: spec.outcome,
    })
}

/// Unloads and deletes the installed unit, so a stopped hub stays stopped
/// across logins. Answers with what it removed.
pub(super) fn remove() -> Result<Option<Installed>, CliError> {
    let Some(installed) = installed() else {
        return Ok(None);
    };
    installed.manager.unload().map_err(|e| {
        CliError::new("service_failed", e).hint(format!("remove {}", installed.unit))
    })?;
    fs::remove_file(&installed.unit).map_err(|e| io_error("removing the service unit", e))?;
    if installed.manager == Manager::Systemd {
        systemd::forget();
    }
    Ok(Some(installed))
}

fn outcome(home: &Home) -> PathBuf {
    home.root().join("hub.outcome")
}

fn user_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
