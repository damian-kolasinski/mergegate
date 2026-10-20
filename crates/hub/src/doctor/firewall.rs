//! Whether the macOS application firewall lets other machines reach the
//! hub's tailnet listener. The firewall keeps a rule per binary, and to it an
//! mg without a Developer ID signature is a new binary on every upgrade: with
//! no rule, connections from the tailnet time out while loopback still works,
//! so nothing else in `mg doctor` notices.

use std::path::{Path, PathBuf};
use std::process::Command;

use mergegate_core::doctor::{Check, CheckStatus};

const SOCKETFILTERFW: &str = "/usr/libexec/ApplicationFirewall/socketfilterfw";

/// What the firewall said, as `socketfilterfw` prints it.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Firewall {
    pub on: bool,
    pub block_all: bool,
    /// "Automatically allow downloaded signed software": a binary with a
    /// valid Developer ID signature needs no rule of its own.
    pub allow_signed: bool,
    pub apps: Vec<App>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct App {
    pub path: PathBuf,
    pub allowed: bool,
}

/// None off macOS, or where the firewall can't be read.
pub(super) async fn check() -> Option<Check> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    tokio::task::spawn_blocking(|| {
        let exe = std::env::current_exe().ok()?;
        let exe = resolve(&exe);
        let mut firewall = Firewall::read()?;
        // The rules name real paths; mise and Homebrew start mg through links.
        for app in &mut firewall.apps {
            app.path = resolve(&app.path);
        }
        judge(&firewall, &exe, developer_id(&exe))
    })
    .await
    .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
}

/// Logs what keeps the tailnet out, for a hub that just started listening
/// there. That is when an upgraded mg finds no rule of its own, and the log
/// is where an operator looks when another machine can't connect.
pub(crate) async fn warn() {
    let Some(check) = check().await else {
        return;
    };
    if check.status != CheckStatus::Ok {
        match check.fix {
            Some(fix) => tracing::warn!("{}; fix: {fix}", check.message),
            None => tracing::warn!("{}", check.message),
        }
    }
}

impl Firewall {
    fn read() -> Option<Self> {
        let ask = |flag: &str| {
            let output = Command::new(SOCKETFILTERFW).arg(flag).output().ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
        };
        Self::parse(
            &ask("--getglobalstate")?,
            &ask("--getblockall")?,
            &ask("--getallowsigned")?,
            &ask("--listapps")?,
        )
    }

    /// None when the global state isn't one this knows how to read.
    pub(super) fn parse(state: &str, block_all: &str, signed: &str, apps: &str) -> Option<Self> {
        // 0 is off; 1 is on, and 2 lets in essential services only, which
        // blocks every app however its rule reads.
        let (on, essential_only) = match state.split("(State = ").nth(1)?.chars().next()? {
            '0' => (false, false),
            '1' => (true, false),
            '2' => (true, true),
            _ => return None,
        };
        Some(Self {
            on,
            block_all: essential_only || block_all.trim_end().ends_with("enabled."),
            allow_signed: signed
                .lines()
                .any(|l| l.contains("downloaded signed software") && l.contains("ENABLED")),
            apps: parse_apps(apps),
        })
    }
}

/// `--listapps` prints each app as `N : PATH`, then its rule on the next line.
fn parse_apps(text: &str) -> Vec<App> {
    let mut apps = Vec::new();
    let mut path: Option<PathBuf> = None;
    for line in text.lines().map(str::trim) {
        if let Some((n, rest)) = line.split_once(" : ")
            && n.chars().all(|c| c.is_ascii_digit())
        {
            path = Some(PathBuf::from(rest.trim()));
        } else if let Some(p) = path.take_if(|_| line.starts_with('(')) {
            apps.push(App {
                path: p,
                allowed: line.contains("Allow incoming connections"),
            });
        }
    }
    apps
}

pub(super) fn judge(firewall: &Firewall, exe: &Path, developer_id: bool) -> Option<Check> {
    if !firewall.on {
        return None;
    }
    let shown = exe.display();
    let (status, message, fix) = if firewall.block_all {
        (
            CheckStatus::Fail,
            "the macOS firewall blocks all incoming connections, so the tailnet can't reach the hub"
                .to_owned(),
            Some(
                "turn off Block all incoming connections in System Settings > Network > Firewall > Options"
                    .to_owned(),
            ),
        )
    } else {
        match firewall.apps.iter().find(|a| a.path == exe) {
            Some(App { allowed: true, .. }) => (
                CheckStatus::Ok,
                format!("the macOS firewall lets the tailnet reach {shown}"),
                None,
            ),
            Some(App { allowed: false, .. }) => (
                CheckStatus::Fail,
                format!("the macOS firewall blocks incoming connections to {shown}"),
                Some(format!("sudo {SOCKETFILTERFW} --unblockapp {}", quote(exe))),
            ),
            None if developer_id && firewall.allow_signed => (
                CheckStatus::Ok,
                format!("the macOS firewall lets the tailnet reach {shown}, as signed software"),
                None,
            ),
            None => (
                CheckStatus::Warn,
                format!(
                    "the macOS firewall has no rule for {shown}, so connections from the tailnet time out"
                ),
                Some(format!(
                    "sudo {SOCKETFILTERFW} --add {exe} && sudo {SOCKETFILTERFW} --unblockapp {exe}",
                    exe = quote(exe)
                )),
            ),
        }
    };
    Some(Check {
        name: "firewall".into(),
        status,
        message,
        fix,
    })
}

/// Signed with a Developer ID that still verifies, which the firewall allows
/// without a rule when it allows downloaded signed software.
fn developer_id(exe: &Path) -> bool {
    let codesign = |args: &[&str]| {
        Command::new("/usr/bin/codesign")
            .args(args)
            .arg(exe)
            .output()
    };
    let verified = codesign(&["--verify"]).is_ok_and(|o| o.status.success());
    verified
        && codesign(&["-dv", "--verbose=2"]).is_ok_and(|o| {
            String::from_utf8_lossy(&o.stderr)
                .lines()
                .any(|l| l.starts_with("Authority=Developer ID Application:"))
        })
}

fn resolve(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned())
}

/// For pasting into a shell.
fn quote(path: &Path) -> String {
    let text = path.display().to_string();
    let plain = |c: char| c.is_ascii_alphanumeric() || "/._-+@~".contains(c);
    if text.chars().all(plain) {
        text
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
#[path = "firewall_tests.rs"]
mod tests;
