//! A LaunchAgent in the user's GUI session.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::Spec;

const LABEL: &str = "dev.mergegate.hub";
const LAUNCHCTL: &str = "/bin/launchctl";

pub(super) fn path(user_home: &Path) -> PathBuf {
    user_home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

pub(super) fn render(spec: &Spec) -> String {
    let args: String = spec
        .args()
        .iter()
        .map(|arg| format!("\t\t<string>{}</string>\n", escape(arg)))
        .collect();
    // `SuccessfulExit` false: a hub stopped through its API or SIGTERM exits
    // 0 and stays stopped; a crash or failed start is retried.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LABEL}</string>
	<key>ProgramArguments</key>
	<array>
{args}	</array>
	<key>EnvironmentVariables</key>
	<dict>
		<key>PATH</key>
		<string>{path}</string>
	</dict>
	<key>WorkingDirectory</key>
	<string>{home}</string>
	<key>StandardOutPath</key>
	<string>{outcome}</string>
	<key>StandardErrorPath</key>
	<string>{log}</string>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
</dict>
</plist>
"#,
        path = escape(&spec.path_env),
        home = escape(&spec.home.display().to_string()),
        outcome = escape(&spec.outcome.display().to_string()),
        log = escape(&spec.log.display().to_string()),
    )
}

/// The binary a plist [`render`] wrote runs.
pub(super) fn exe(plist: &str) -> Option<PathBuf> {
    let (_, rest) = plist.split_once("<key>ProgramArguments</key>")?;
    let (_, rest) = rest.split_once("<string>")?;
    let (exe, _) = rest.split_once("</string>")?;
    Some(PathBuf::from(unescape(exe)))
}

/// Loads the agent, which starts the hub. One loaded from an unchanged plist
/// is only started: unloading would stop a hub it is still starting.
pub(super) fn load(plist: &Path, changed: bool) -> Result<(), String> {
    let target = target();
    let loaded = launchctl(&["print", &target]).is_ok();
    if loaded && !changed {
        return launchctl(&["kickstart", &target]);
    }
    if loaded {
        launchctl(&["bootout", &target])?;
        gone(&target);
    }
    launchctl(&["bootstrap", &domain(), &plist.display().to_string()])
}

/// Waits for launchd to drop a job `bootout` asked to go: `bootout` returns
/// before it has, and bootstrapping the label meanwhile fails with an I/O
/// error. Gives up after a few seconds and lets the bootstrap say so.
fn gone(target: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while launchctl(&["print", target]).is_ok() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
}

/// Unloads the agent, stopping a hub it runs. Not loaded is fine.
pub(super) fn unload() -> Result<(), String> {
    let target = target();
    if launchctl(&["print", &target]).is_err() {
        return Ok(());
    }
    launchctl(&["bootout", &target])
}

#[allow(unsafe_code)]
fn domain() -> String {
    // SAFETY: getuid has no preconditions and can't fail.
    let uid = unsafe { libc::getuid() };
    format!("gui/{uid}")
}

fn target() -> String {
    format!("{}/{LABEL}", domain())
}

fn launchctl(args: &[&str]) -> Result<(), String> {
    let output = Command::new(LAUNCHCTL)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("can't run launchctl: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "launchctl {} failed: {}",
        args[0],
        stderr.trim().lines().next().unwrap_or_default()
    ))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
#[path = "launchd_tests.rs"]
mod tests;
