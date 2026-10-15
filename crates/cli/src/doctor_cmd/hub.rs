//! Whether the hub runs, and runs this `mg`'s code.

use api::doctor::{Check, CheckStatus};

pub(super) fn check(url: &str, version: &str) -> Check {
    versioned(url, version, env!("CARGO_PKG_VERSION"))
}

fn versioned(url: &str, version: &str, ours: &str) -> Check {
    // An API both speak still leaves fixes in this mg the hub lacks.
    if version == ours {
        return Check {
            name: "hub".into(),
            status: CheckStatus::Ok,
            message: format!("running at {url}"),
            fix: None,
        };
    }
    Check {
        name: "hub".into(),
        status: CheckStatus::Warn,
        message: format!("running at {url}, but as mg {version}; this is mg {ours}"),
        fix: Some("mg hub stop && mg hub start".into()),
    }
}

#[cfg(test)]
#[path = "hub_tests.rs"]
mod tests;
