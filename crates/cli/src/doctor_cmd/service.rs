//! Whether the default home's hub has a service that starts this `mg`.

use std::fs;
use std::path::Path;

use api::doctor::{Check, CheckStatus};

use crate::hub_cmd::service::{self, Installed};

/// None where the hub gets no service: another home, or no manager to run it.
pub(super) fn check(hub_running: bool) -> Option<Check> {
    if !service::applies() {
        return None;
    }
    let installed = service::installed(service::Role::Hub);
    if installed.is_none() && !service::usable() {
        return None;
    }
    let this = std::env::current_exe().ok();
    Some(judge(installed.as_ref(), this.as_deref(), hub_running))
}

fn judge(installed: Option<&Installed>, this: Option<&Path>, hub_running: bool) -> Check {
    let restart = "mg hub stop && mg hub start";
    let Some(installed) = installed else {
        return Check {
            name: "service".into(),
            status: CheckStatus::Warn,
            message: "the hub has no service, so it won't start at login".into(),
            fix: Some(if hub_running { restart } else { "mg hub start" }.into()),
        };
    };
    let warn = |message: String| Check {
        name: "service".into(),
        status: CheckStatus::Warn,
        message,
        fix: Some(restart.into()),
    };
    let Some(exe) = &installed.exe else {
        return warn(format!("{} isn't a unit mg wrote", installed.unit));
    };
    if let Some(this) = this
        && !same_file(Path::new(exe), this)
    {
        return warn(format!(
            "the hub's service runs {exe}, not this mg ({})",
            this.display()
        ));
    }
    Check {
        name: "service".into(),
        status: CheckStatus::Ok,
        message: format!(
            "{} starts the hub at login: {}",
            installed.manager.name(),
            installed.unit
        ),
        fix: None,
    }
}

/// Links and relative spellings resolved where the files exist; a Homebrew
/// `mg` is a link into its versioned cellar.
pub(super) fn same_file(a: &Path, b: &Path) -> bool {
    let resolve = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_owned());
    resolve(a) == resolve(b)
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
