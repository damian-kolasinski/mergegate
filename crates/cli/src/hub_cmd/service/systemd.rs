//! A systemd user service.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::Spec;

const UNIT: &str = "mergegate-hub.service";

/// `$XDG_CONFIG_HOME/systemd/user/`, or `~/.config/systemd/user/`.
pub(super) fn path(user_home: &Path) -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| user_home.join(".config"));
    config.join("systemd/user").join(UNIT)
}

/// Whether this session has a user manager to talk to; containers and WSL
/// often don't.
pub(super) fn usable() -> bool {
    systemctl(&["show-environment"]).is_ok()
}

pub(super) fn render(spec: &Spec) -> String {
    let exec: Vec<String> = spec
        .args()
        .iter()
        .map(|arg| quote(arg).replace('$', "$$"))
        .collect();
    // `on-failure`: a hub stopped through its API or SIGTERM exits 0 and
    // stays stopped; a crash or failed start is retried.
    format!(
        "[Unit]
Description=mergegate hub

[Service]
ExecStart={exec}
Environment={path}
WorkingDirectory={home}
StandardOutput=append:{outcome}
StandardError=append:{log}
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
",
        exec = exec.join(" "),
        path = quote(&format!("PATH={}", spec.path_env)),
        home = specifiers(&spec.home.display().to_string()),
        outcome = specifiers(&spec.outcome.display().to_string()),
        log = specifiers(&spec.log.display().to_string()),
    )
}

/// The binary a unit [`render`] wrote runs.
pub(super) fn exe(unit: &str) -> Option<PathBuf> {
    let exec = unit
        .lines()
        .find_map(|line| line.strip_prefix("ExecStart="))?;
    let mut chars = exec.strip_prefix('"')?.chars();
    let mut exe = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(PathBuf::from(exe.replace("%%", "%").replace("$$", "$"))),
            '\\' => exe.push(chars.next()?),
            c => exe.push(c),
        }
    }
    None
}

/// Enables and starts the service. Reloading picks up a changed unit, and
/// resetting a failed one lets it start again after its restart limit.
pub(super) fn load(_unit: &Path, _changed: bool) -> Result<(), String> {
    systemctl(&["daemon-reload"])?;
    let _ = systemctl(&["reset-failed", UNIT]);
    systemctl(&["enable", UNIT])?;
    systemctl(&["start", UNIT])
}

/// Disables and stops the service. Not loaded is fine.
pub(super) fn unload() -> Result<(), String> {
    let _ = systemctl(&["disable", "--now", UNIT]);
    Ok(())
}

/// Stops the service, leaving it enabled for the next login.
pub(super) fn stop() {
    let _ = systemctl(&["stop", UNIT]);
}

/// Forgets a removed unit file.
pub(super) fn forget() {
    let _ = systemctl(&["daemon-reload"]);
}

fn systemctl(args: &[&str]) -> Result<(), String> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("can't run systemctl: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "systemctl --user {} failed: {}",
        args[0],
        stderr.trim().lines().next().unwrap_or_default()
    ))
}

/// A word systemd reads back as `text`: quoted, with C escapes, and `%`
/// doubled so no specifier expands. Command lines also expand `$`, so they
/// double it too.
fn quote(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

/// A path setting, where only `%` specifiers expand.
fn specifiers(text: &str) -> String {
    text.replace('%', "%%")
}

#[cfg(test)]
#[path = "systemd_tests.rs"]
mod tests;
